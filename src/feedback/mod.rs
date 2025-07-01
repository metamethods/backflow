//! Feedback module
//! "Feedback" in this case is used to handle feedback messages sent back
//! to the HID device, such as LED control, haptics, etc.
//!
use crossbeam::channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};

pub mod websocket;

/// Represents a packet of feedback events, sent over a network or any other communication channel.
/// (i.e WebSocket, Unix Domain Socket, etc.)
///W
/// The packet contains a device identifier, timestamp and a list of feedback events to be processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEventPacket {
    /// The device identifier for this packet, used for routing events.
    pub device_id: String,

    /// The timestamp of the packet, in epoch milliseconds.
    pub timestamp: u64,

    /// List of feedback events that occurred in this packet.
    pub events: Vec<FeedbackEvent>,
}

/*
Example JSON for FeedbackEventPacket:

{
    "device_id": "controller-001",
    "timestamp": 1718040000000,
    "events": [
        {
            "Led": {
                "Set": {
                    "led_id": 1,
                    "on": true,
                    "brightness": 200,
                    "rgb": null
                }
            }
        },
        {
            "Led": {
                "Set": {
                    "led_id": 2,
                    "on": true,
                    "brightness": null,
                    "rgb": [255, 100, 50]
                }
            }
        },
        {
            "Led": {
                "SetPattern": {
                    "led_id": 3,
                    "pattern": {
                        "Blink": {
                            "interval_ms": 300
                        }
                    }
                }
            }
        },
        {
            "Haptic": {
                "Vibrate": {
                    "motor_id": 0,
                    "intensity": 128,
                    "duration_ms": 500
                }
            }
        },
        {
            "Haptic": {
                "StopVibration": {
                    "motor_id": 0
                }
            }
        },
        {
            "Haptic": {
                "ForceFeedback": {
                    "motors": [
                        [0, 200],
                        [1, 150]
                    ],
                    "duration_ms": 1000
                }
            }
        }
    ]
}
*/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackEvent {
    /// LED control event, such as turning on/off or changing brightness.
    Led(LedEvent),
    /// Haptic feedback event, such as vibration patterns.
    Haptic(HapticEvent),
}

#[async_trait::async_trait]
pub trait FeedbackBackend: Send {
    /// Starts the feedback backend, processing feedback events and sending them to the appropriate device.
    async fn run(&mut self) -> eyre::Result<()>;
}

/// LED control events for controlling device LEDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedEvent {
    /// Set the state of an LED: on/off, brightness, and optional RGB color.
    Set {
        /// The LED ID to control.
        led_id: u8,
        /// Whether the LED should be on (true) or off (false).
        on: bool,
        /// Optional brightness (0-255). If None, use device default.
        ///
        /// If `on` is false, this value is ignored.
        brightness: Option<u8>,
        /// Optional RGB color. If None, use device default or white.
        rgb: Option<(u8, u8, u8)>,
    },
    /// Set a pattern for the LED (e.g., blinking, breathing).
    SetPattern { led_id: u8, pattern: LedPattern },
}

/// LED patterns for controlling LED behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedPattern {
    /// Solid color (no pattern).
    Solid,
    /// Blinking pattern with specified interval in milliseconds.
    Blink { interval_ms: u32 },
    /// Breathing pattern with specified duration in milliseconds.
    Breathe { duration_ms: u32 },
    /// Fade in/out pattern.
    Fade { fade_in_ms: u32, fade_out_ms: u32 },
}

/// Haptic feedback events for controlling vibration and force feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HapticEvent {
    /// Start vibration on a specific motor with specified intensity (0-255) and duration.
    Vibrate {
        /// The motor ID to vibrate.
        motor_id: u8,
        /// Intensity (0-255).
        intensity: u8,
        /// Duration in milliseconds.
        duration_ms: u32,
    },
    /// Stop vibration on a specific motor.
    StopVibration {
        /// The motor ID to stop.
        motor_id: u8,
    },
    /// Force feedback for multiple motors (e.g., joysticks/controllers).
    ForceFeedback {
        /// List of motor IDs and their intensities (0-255).
        motors: Vec<(u8, u8)>,
        /// Duration in milliseconds.
        duration_ms: u32,
    },
}

impl FeedbackEventPacket {
    /// Creates a new `FeedbackEventPacket` with the given device ID and timestamp.
    pub fn new(device_id: String, timestamp: u64) -> Self {
        Self {
            device_id,
            timestamp,
            events: Vec::new(),
        }
    }

    /// Adds an event to the packet.
    pub fn add_event(&mut self, event: FeedbackEvent) {
        self.events.push(event);
    }
}

/// Represents a stream of feedback event packets, using crossbeam channels for communication.
#[derive(Clone)]
pub struct FeedbackEventStream {
    /// Sender for feedback event packets.
    pub tx: Sender<FeedbackEventPacket>,
    /// Receiver for feedback event packets.
    pub rx: Receiver<FeedbackEventPacket>,
}

impl FeedbackEventStream {
    /// Creates a new `FeedbackEventStream` with a crossbeam channel.
    pub fn new() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self { tx, rx }
    }

    /// Sends a feedback event packet through the stream.
    pub fn send(
        &self,
        packet: FeedbackEventPacket,
    ) -> Result<(), crossbeam::channel::SendError<FeedbackEventPacket>> {
        self.tx.send(packet)
    }

    /// Receives a feedback event packet from the stream.
    pub fn receive(&self) -> Result<FeedbackEventPacket, crossbeam::channel::RecvError> {
        self.rx.recv()
    }
}

impl Default for FeedbackEventStream {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_event_packet_serialization() {
        let mut packet = FeedbackEventPacket::new("controller-001".to_string(), 1718040000000);
        packet.add_event(FeedbackEvent::Led(LedEvent::Set {
            led_id: 1,
            on: true,
            brightness: Some(200),
            rgb: None,
        }));
        packet.add_event(FeedbackEvent::Haptic(HapticEvent::Vibrate {
            motor_id: 0,
            intensity: 128,
            duration_ms: 500,
        }));

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: FeedbackEventPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(packet.device_id, deserialized.device_id);
        assert_eq!(packet.timestamp, deserialized.timestamp);
        assert_eq!(packet.events.len(), deserialized.events.len());
    }

    #[test]
    fn test_led_event_setpattern_blink() {
        let event = FeedbackEvent::Led(LedEvent::SetPattern {
            led_id: 3,
            pattern: LedPattern::Blink { interval_ms: 300 },
        });
        if let FeedbackEvent::Led(LedEvent::SetPattern { led_id, pattern }) = event {
            assert_eq!(led_id, 3);
            match pattern {
                LedPattern::Blink { interval_ms } => assert_eq!(interval_ms, 300),
                _ => panic!("Expected Blink pattern"),
            }
        } else {
            panic!("Expected Led SetPattern event");
        }
    }

    #[test]
    fn test_haptic_force_feedback() {
        let event = FeedbackEvent::Haptic(HapticEvent::ForceFeedback {
            motors: vec![(0, 200), (1, 150)],
            duration_ms: 1000,
        });
        if let FeedbackEvent::Haptic(HapticEvent::ForceFeedback {
            motors,
            duration_ms,
        }) = event
        {
            assert_eq!(motors, vec![(0, 200), (1, 150)]);
            assert_eq!(duration_ms, 1000);
        } else {
            panic!("Expected Haptic ForceFeedback event");
        }
    }

    #[test]
    fn test_feedback_event_stream_send_receive() {
        let stream = FeedbackEventStream::new();
        let mut packet = FeedbackEventPacket::new("dev".to_string(), 12345);
        packet.add_event(FeedbackEvent::Led(LedEvent::Set {
            led_id: 2,
            on: false,
            brightness: None,
            rgb: Some((255, 0, 0)),
        }));

        stream.send(packet.clone()).unwrap();
        let received = stream.receive().unwrap();
        assert_eq!(received.device_id, "dev");
        assert_eq!(received.events.len(), 1);
    }

    #[test]
    fn test_led_event_set_with_rgb() {
        let event = LedEvent::Set {
            led_id: 5,
            on: true,
            brightness: Some(100),
            rgb: Some((10, 20, 30)),
        };
        if let LedEvent::Set {
            led_id,
            on,
            brightness,
            rgb,
        } = event
        {
            assert_eq!(led_id, 5);
            assert!(on);
            assert_eq!(brightness, Some(100));
            assert_eq!(rgb, Some((10, 20, 30)));
        } else {
            panic!("Expected LedEvent::Set");
        }
    }

    #[test]
    fn test_led_pattern_breathe() {
        let pattern = LedPattern::Breathe { duration_ms: 1500 };
        match pattern {
            LedPattern::Breathe { duration_ms } => assert_eq!(duration_ms, 1500),
            _ => panic!("Expected Breathe pattern"),
        }
    }

    #[test]
    fn test_haptic_stop_vibration() {
        let event = HapticEvent::StopVibration { motor_id: 2 };
        match event {
            HapticEvent::StopVibration { motor_id } => assert_eq!(motor_id, 2),
            _ => panic!("Expected StopVibration"),
        }
    }
}
