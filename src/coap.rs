use anyhow::{Context, Result};
use coap_lite::{CoapRequest, MessageClass, Packet, RequestType};
use openssl::ssl::{Ssl, SslContext, SslMethod, SslOptions, SslVerifyMode};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const COAP_PORT: u16 = 5684;
const BUF_SIZE: usize = 4096;
const TIMEOUT_SECS: u64 = 10;

/// Light info parsed from Trådfri gateway response
#[derive(Debug, Clone, Deserialize)]
pub struct LightInfo {
    pub id: u64,
    pub name: String,
    pub on: bool,
    pub brightness: u8,
    pub color_hex: Option<String>,
    pub reachable: bool,
}

/// Raw Trådfri device JSON (keys are CoAP resource numbers)
#[derive(Debug, Deserialize)]
struct TradfriDevice {
    /// Device info
    #[serde(rename = "3")]
    _info: Option<serde_json::Value>,
    /// Light list (array of bulbs)
    #[serde(rename = "3311")]
    bulbs: Option<Vec<TradfriLightBulb>>,
    /// Device name
    #[serde(rename = "9001")]
    name: String,
    /// Instance ID
    #[serde(rename = "9003")]
    id: u64,
    /// Reachable (1 = true, 0 = false)
    #[serde(rename = "9019")]
    reachable: Option<u32>,
    /// Device type
    #[serde(rename = "5750", default)]
    _device_type: u32,
}

#[derive(Debug, Deserialize)]
struct TradfriLightBulb {
    /// Color hex (e.g. "f1e0b5")
    #[serde(rename = "5706")]
    color_hex: Option<String>,
    /// On/Off (1/0)
    #[serde(rename = "5850")]
    on: Option<u32>,
    /// Brightness (0-254)
    #[serde(rename = "5851")]
    brightness: Option<u8>,
}

/// UDP channel that implements Read/Write for openssl
#[derive(Debug)]
struct UdpChannel {
    socket: UdpSocket,
    remote_addr: SocketAddr,
}

impl Read for UdpChannel {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.socket.recv(buf)
    }
}

impl Write for UdpChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.socket.send_to(buf, self.remote_addr)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Persistent DTLS stream over UDP for CoAP communication.
struct DtlsCoap {
    host: String,
    identity: String,
    psk: String,
    stream: Option<openssl::ssl::SslStream<UdpChannel>>,
    msg_id: u16,
}

impl DtlsCoap {
    fn new(host: &str, identity: &str, psk: &str) -> Result<Self> {
        let mut this = Self {
            host: host.to_string(),
            identity: identity.to_string(),
            psk: psk.to_string(),
            stream: None,
            msg_id: 1,
        };
        this.ensure_connected()?;
        Ok(this)
    }

    fn ensure_connected(&mut self) -> Result<()> {
        if self.stream.is_none() {
            self.stream = Some(Self::connect_stream(&self.host, &self.identity, &self.psk)?);
        }
        Ok(())
    }

    /// Connect to Trådfri gateway via DTLS/PSK
    fn connect_stream(
        host: &str,
        identity: &str,
        psk: &str,
    ) -> Result<openssl::ssl::SslStream<UdpChannel>> {
        let addr: SocketAddr = format!("{}:{}", host, COAP_PORT)
            .parse()
            .context("Invalid gateway address")?;

        let socket = UdpSocket::bind("0.0.0.0:0").context("Failed to bind UDP socket")?;
        socket.set_read_timeout(Some(Duration::from_secs(TIMEOUT_SECS)))?;
        socket.set_write_timeout(Some(Duration::from_secs(TIMEOUT_SECS)))?;
        socket.connect(addr)?;

        let channel = UdpChannel {
            socket,
            remote_addr: addr,
        };

        let identity_bytes = identity.as_bytes().to_vec();
        let psk_bytes = psk.as_bytes().to_vec();

        let mut ctx = SslContext::builder(SslMethod::dtls())
            .context("Failed to create DTLS context")?;

        // OpenSSL 3 kan blockera legacy PSK-ciphers via security level.
        // Trådfri gateway kräver PSK-AES128-CCM8 över DTLS.
        ctx.set_cipher_list("PSK-AES128-CCM8:@SECLEVEL=0")
            .context("Failed to set cipher")?;
        ctx.set_verify(SslVerifyMode::NONE);
        ctx.set_options(SslOptions::ALLOW_UNSAFE_LEGACY_RENEGOTIATION);

        ctx.set_psk_client_callback(move |_ssl, _hint, mut identity_buf, mut psk_buf| {
            identity_buf.write_all(&identity_bytes).ok();
            psk_buf.write_all(&psk_bytes).ok();
            Ok(psk_bytes.len())
        });

        let ssl_ctx = ctx.build();
        let mut ssl = Ssl::new(&ssl_ctx).context("Failed to create SSL instance")?;
        ssl.set_connect_state();

        ssl.connect(channel)
            .map_err(|e| anyhow::anyhow!("DTLS handshake failed: {:?}", e))
    }

    fn request(&mut self, request: Packet) -> Result<Packet> {
        let bytes = request
            .to_bytes()
            .context("Failed to serialize CoAP request")?;

        // Retry once with a fresh DTLS session if the persistent stream breaks.
        let mut last_err = None;
        for _ in 0..2 {
            self.ensure_connected()?;

            let response = (|| -> Result<Packet> {
                let stream = self
                    .stream
                    .as_mut()
                    .context("DTLS stream is not connected")?;

                stream.write_all(&bytes)?;

                let mut buf = [0u8; BUF_SIZE];
                let len = stream.read(&mut buf)?;

                Packet::from_bytes(&buf[..len]).context("Failed to parse CoAP response")
            })();

            match response {
                Ok(packet) => return Ok(packet),
                Err(e) => {
                    last_err = Some(e);
                    self.stream = None;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("DTLS request failed")))
    }

    /// Send a CoAP GET request
    fn get(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        request.set_method(RequestType::Get);
        request.set_path(path);
        request.message.header.message_id = self.next_msg_id();

        let response = self.request(request.message)?;

        match response.header.code {
            MessageClass::Response(ref code) => {
                use coap_lite::ResponseType::*;
                match code {
                    Content | Created | Changed | Deleted | Valid => {}
                    _ => {
                        anyhow::bail!(
                            "CoAP error {:?}: {}",
                            code,
                            String::from_utf8_lossy(&response.payload)
                        );
                    }
                }
            }
            _ => {}
        }

        Ok(response.payload)
    }

    /// Send a CoAP PUT request with JSON payload
    fn put(&mut self, path: &str, payload: &[u8]) -> Result<()> {
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        request.set_method(RequestType::Put);
        request.set_path(path);
        request.message.header.message_id = self.next_msg_id();
        request.message.payload = payload.to_vec();

        let response = self.request(request.message)?;

        match response.header.code {
            MessageClass::Response(ref code) => {
                use coap_lite::ResponseType::*;
                match code {
                    Content | Created | Changed | Deleted | Valid => Ok(()),
                    _ => {
                        anyhow::bail!(
                            "CoAP PUT error {:?}: {}",
                            code,
                            String::from_utf8_lossy(&response.payload)
                        );
                    }
                }
            }
            _ => Ok(()),
        }
    }

    fn next_msg_id(&mut self) -> u16 {
        let id = self.msg_id;
        self.msg_id = self.msg_id.wrapping_add(1);
        id
    }
}

/// Trådfri client using persistent DTLS connection
pub struct TradfriClient {
    coap: DtlsCoap,
}

impl TradfriClient {
    pub fn new(host: &str, identity: &str, psk: &str) -> Result<Self> {
        let coap = DtlsCoap::new(host, identity, psk)
            .context("Failed to connect to Trådfri gateway")?;
        Ok(Self { coap })
    }

    /// List all lights from the gateway
    pub fn list_lights(&mut self) -> Result<Vec<LightInfo>> {
        // Get device IDs
        let payload = self.coap.get("15001")?;
        let ids: Vec<u64> = serde_json::from_slice(&payload)
            .context("Failed to parse device ID list")?;

        let mut lights = Vec::new();
        for id in ids {
            if let Ok(light) = self.get_light(id) {
                lights.push(light);
            }
        }
        Ok(lights)
    }

    /// Get a single light's info
    fn get_light(&mut self, id: u64) -> Result<LightInfo> {
        let payload = self.coap.get(&format!("15001/{}", id))?;
        let device: TradfriDevice = serde_json::from_slice(&payload)
            .context("Failed to parse device")?;

        // Only return if it has bulbs (is a light)
        let bulb = device
            .bulbs
            .as_ref()
            .and_then(|b| b.first())
            .context("Not a light device")?;

        Ok(LightInfo {
            id: device.id,
            name: device.name,
            on: bulb.on.unwrap_or(0) == 1,
            brightness: bulb.brightness.unwrap_or(0),
            color_hex: bulb.color_hex.clone(),
            reachable: device.reachable.unwrap_or(0) == 1,
        })
    }

    /// Set power on/off for a light.
    pub fn set_power(&mut self, id: u64, on: bool) -> Result<()> {
        let payload = serde_json::json!({
            "3311": [{"5850": if on { 1 } else { 0 }}]
        });
        self.coap
            .put(&format!("15001/{}", id), payload.to_string().as_bytes())
    }

    /// Set brightness (0-254)
    pub fn set_brightness(&mut self, id: u64, brightness: u8) -> Result<()> {
        let payload = serde_json::json!({
            "3311": [{"5851": brightness, "5850": if brightness > 0 { 1 } else { 0 }}]
        });
        self.coap
            .put(&format!("15001/{}", id), payload.to_string().as_bytes())
    }

    /// Set color temperature by hex value
    pub fn set_color(&mut self, id: u64, hex: &str) -> Result<()> {
        let payload = serde_json::json!({
            "3311": [{"5706": hex}]
        });
        self.coap
            .put(&format!("15001/{}", id), payload.to_string().as_bytes())
    }

    /// Apply a scene (set brightness + color + on/off for a light)
    pub fn apply_scene_to_light(
        &mut self,
        id: u64,
        on: bool,
        brightness: u8,
        color_hex: &str,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "3311": [{
                "5850": if on { 1 } else { 0 },
                "5851": brightness,
                "5706": color_hex
            }]
        });
        self.coap
            .put(&format!("15001/{}", id), payload.to_string().as_bytes())
    }
}

/// Thread-safe wrapper for TradfriClient
#[derive(Clone)]
pub struct SharedTradfriClient {
    inner: Arc<Mutex<TradfriClient>>,
}

impl SharedTradfriClient {
    pub fn new(host: &str, identity: &str, psk: &str) -> Result<Self> {
        let client = TradfriClient::new(host, identity, psk)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(client)),
        })
    }

    pub fn list_lights(&self) -> Result<Vec<LightInfo>> {
        self.inner.lock().unwrap().list_lights()
    }

    pub fn set_power(&self, id: u64, on: bool) -> Result<()> {
        self.inner.lock().unwrap().set_power(id, on)
    }

    pub fn set_brightness(&self, id: u64, brightness: u8) -> Result<()> {
        self.inner.lock().unwrap().set_brightness(id, brightness)
    }

    pub fn set_color(&self, id: u64, hex: &str) -> Result<()> {
        self.inner.lock().unwrap().set_color(id, hex)
    }

    pub fn apply_scene_to_light(
        &self,
        id: u64,
        on: bool,
        brightness: u8,
        color_hex: &str,
    ) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .apply_scene_to_light(id, on, brightness, color_hex)
    }
}
