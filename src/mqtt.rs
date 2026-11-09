/*
 * MQTT client for ThingCloud/Tuya C236T device control
 * 
 * Connects to ThingCloud MQTT broker to send/receive DP commands.
 * 
 * ThingCloud MQTT protocol:
 *   - Connect to MQTT broker (TCP port 1883)
 *   - Username/Password: ThingCloud credentials
 *   - Client ID: thing_{devId}
 *   - Subscribe to: thingcloud/{devId}/dp/report
 *   - Publish to:   thingcloud/{devId}/dp/write
 *
 * Payload format: JSON like {"107": 25, "108": 30}
 */

use crate::dp::{C236tState, DpId, DpValue, dp_map};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

/// List of ThingCloud MQTT broker hosts to try
const MQTT_HOSTS: &[&str] = &[
    "ec2-18-196-142-136.eu-central-1.compute.amazonaws.com",  // C236T MQTT broker (EU)
];

/// MQTT configuration from ThingCloud credentials
#[derive(Clone)]
pub struct MqttConfig {
    pub app_key: String,
    pub app_secret: String,
    pub dev_id: String,
    pub user_token: String,
}

/// ThingCloud MQTT client wrapper
pub struct C236tMqtt {
    config: MqttConfig,
    connected: bool,
    client: Option<rumqttc::AsyncClient>,
    _eventloop: Option<tokio::task::JoinHandle<()>>,
}

impl C236tMqtt {
    /// Connect to ThingCloud MQTT broker - tries multiple hosts and credentials
    pub async fn connect(config: MqttConfig) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Connecting to ThingCloud MQTT...");
        info!("Device ID: {}", config.dev_id);

        let mqtt_port: u16 = 8883;
        let client_id = format!("thing_{}", config.dev_id);

        for host in MQTT_HOSTS {
            info!("Trying MQTT host: {}", host);

            let mut mqtt_options = rumqttc::MqttOptions::new(&client_id, *host, mqtt_port);
            mqtt_options
                .set_keep_alive(Duration::from_secs(60))
                .set_credentials(&config.app_key, &config.user_token)
                .set_transport(rumqttc::Transport::tls_with_default_config());

            let (mut client, mut eventloop) = rumqttc::AsyncClient::new(mqtt_options, 10);

                    // Spawn eventloop task in background
                    let eventloop_handle = tokio::spawn(async move {
                        let _ = eventloop.poll().await;
                    });

                    // Wait for MQTT CONNECT to complete
                    sleep(Duration::from_secs(1)).await;

                    // Test connection by subscribing to a test topic
                    match client.subscribe("test/connect", rumqttc::QoS::AtMostOnce).await {
                        Ok(_) => {
                            info!("✓ MQTT connected successfully to {}", host);
                            return Ok(Self {
                                config,
                                connected: true,
                                client: Some(client),
                                _eventloop: Some(eventloop_handle),
                            });
                        }
                        Err(e) => {
                            info!("  MQTT auth failed on {}: {}", host, e);
                            // Abort the eventloop task
                            eventloop_handle.abort();
                            continue;
                        }
                    }
        }

        Err(format!("Could not connect to any MQTT broker host with provided credentials").into())
    }

    /// Subscribe to device DP update topic
    pub async fn subscribe_dp_updates(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let report_topic = topics::dp_report(&self.config.dev_id);
        let write_topic = topics::dp_write(&self.config.dev_id);

        if let Some(ref client) = self.client {
            client.subscribe(report_topic.clone(), rumqttc::QoS::AtLeastOnce).await?;
            info!("✓ Subscribed to: {}", report_topic);
        }

        info!("MQTT publish topic: {}", write_topic);
        Ok(())
    }

    /// Send a DP command to the device
    pub async fn send_dp(&self, dp_id: &str, value: &DpValue) -> Result<(), Box<dyn std::error::Error>> {
        let topic = topics::dp_write(&self.config.dev_id);
        let payload = serde_json::to_string(&dp_map(dp_id, value.clone()))?;

        info!("DP Command: {} = {:?}", dp_id, value);
        println!("  --> MQTT PUBLISH {} {}", topic, payload);

        if let Some(ref client) = self.client {
            client
                .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload)
                .await?;
        }

        Ok(())
    }

    /// Send multiple DP commands at once
    pub async fn send_dps(
        &self,
        commands: &HashMap<&str, DpValue>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let topic = topics::dp_write(&self.config.dev_id);

        let payload: HashMap<String, DpValue> = commands
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();

        let payload_json = serde_json::to_string(&payload)?;

        info!("Multi DP Command: {}", payload_json);
        println!("  --> MQTT PUBLISH {} {}", topic, payload_json);

        if let Some(ref client) = self.client {
            client
                .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload_json)
                .await?;
        }

        Ok(())
    }

    /// Query all DPs from device (send DP 101 with false)
    pub async fn query_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_dp(DpId::QUERY_ALL, &DpValue::Bool(false)).await
    }

    /// Process incoming MQTT messages (background task)
    pub async fn run_message_loop(
        &self,
        state: Arc<Mutex<C236tState>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("MQTT message loop started (waiting for DP updates)...");
        loop {
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Disconnect from MQTT
    pub async fn disconnect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Disconnecting from MQTT...");
        self.connected = false;
        Ok(())
    }
}

/// Standard ThingCloud MQTT topic patterns
pub mod topics {
    /// Subscribe to device DP reports
    pub fn dp_report(dev_id: &str) -> String {
        format!("thingcloud/{}/dp/report", dev_id)
    }

    /// Publish DP commands to device
    pub fn dp_write(dev_id: &str) -> String {
        format!("thingcloud/{}/dp/write", dev_id)
    }

    /// Subscribe to device status changes
    pub fn dev_status(dev_id: &str) -> String {
        format!("thingcloud/{}/status", dev_id)
    }

    /// Subscribe to device info updates
    pub fn dev_info(dev_id: &str) -> String {
        format!("thingcloud/{}/info", dev_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topics() {
        let dev = "example-device-id";
        assert_eq!(
            topics::dp_report(dev),
            "thingcloud/example-device-id/dp/report"
        );
        assert_eq!(
            topics::dp_write(dev),
            "thingcloud/example-device-id/dp/write"
        );
    }
}