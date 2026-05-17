use base64::{Engine, prelude::BASE64_STANDARD};
use rand::RngExt;
use sha1::{Digest, Sha1};

use reqwest::header::CONTENT_TYPE;

use url::Url;

use crate::config::CameraConfig;

const NS_SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const NS_DEVICE: &str = "http://www.onvif.org/ver10/device/wsdl";
const NS_SCHEMA: &str = "http://www.onvif.org/ver10/schema";
const NS_MEDIA: &str = "http://www.onvif.org/ver10/media/wsdl";
const NS_PTZ: &str = "http://www.onvif.org/ver20/ptz/wsdl";

/// ONVIF PTZ client for a single camera.
///
/// ONVIF is an industry standard for IP camera interoperability. It exposes camera
/// functionality (PTZ, media profiles, device info) as SOAP services — HTTP POST
/// requests with XML bodies.
///
/// # How it works
///
/// Every PTZ command follows this flow:
///
/// 1. **Authenticate** — ONVIF rejects plain passwords. Every request must include a
///    WS-Security header containing a digest: `Base64(SHA-1(nonce + timestamp + password))`.
///    The nonce is random bytes generated fresh per request. [`OnvifClient`] builds this
///    header automatically via a private method.
///
/// 2. **Discover service URLs** — ONVIF does not require services to live at fixed paths.
///    [`OnvifClient::connect`] calls `GetCapabilities` on the Device service at construction
///    time. The response contains the actual URLs (`XAddr`) for each service on this specific
///    camera. These are stored on the struct and reused for all subsequent calls. This is
///    what makes the client work across different camera brands without hardcoding paths.
///
/// 3. **Get a profile token** — a camera can expose multiple *media profiles*, each
///    representing a different stream configuration (e.g. "MainStream" at full resolution,
///    "SubStream" at low resolution for mobile). Each profile has a unique token string
///    chosen by the manufacturer. PTZ commands require you to specify which profile you
///    are targeting.
///
///    In practice, all profiles on the same camera move the same physical lens, so it
///    doesn't matter which profile token you use for PTZ. [`OnvifClient::connect`] fetches
///    the first profile token automatically and stores it on the struct — callers never
///    need to manage tokens directly.
///
/// 4. **Send PTZ commands** — call [`OnvifClient::ptz_move`] to start panning/tilting
///    and [`OnvifClient::ptz_stop`] to halt. Both use the stored profile token internally.
///
/// # Construction
///
/// Use [`OnvifClient::connect`] (async) — it runs `GetCapabilities` and `GetProfiles`,
/// then returns a fully initialised client ready to send PTZ commands:
///
/// ```ignore
/// let client = OnvifClient::connect(camera_config).await?;
/// client.ptz_move(0.5, 0.0).await?;
/// client.ptz_stop().await?;
/// ```
pub struct OnvifClient {
    client: reqwest::Client,
    camera_config: CameraConfig,
    media_service_url: String,
    ptz_service_url: String,
    profile_token: String,
}

impl OnvifClient {
    pub async fn connect(camera_config: CameraConfig) -> anyhow::Result<Self> {
        let client = reqwest::Client::new();

        let (media_service_url, ptz_service_url) =
            get_service_urls(&client, &camera_config).await?;
        let profile_token = get_profile_token(&client, &media_service_url, &camera_config).await?;

        Ok(OnvifClient {
            client,
            camera_config,
            media_service_url,
            ptz_service_url,
            profile_token,
        })
    }

    pub async fn ptz_move(&self, pan: f32, tilt: f32) -> anyhow::Result<()> {
        // See example request in Annex B, at B.5.3.1 in the
        // ONVIF Application Programmers Guide
        let body = format!(
            r#"
        <tptz:ContinuousMove 
            xmlns:tptz="{NS_PTZ}" 
            xmlns:tt="{NS_SCHEMA}">
            <tptz:ProfileToken>{token}</tptz:ProfileToken>
            <tptz:Velocity>
                <tt:PanTilt x="{pan}" y="{tilt}"/>
            </tptz:Velocity>
        </tptz:ContinuousMove>
        "#,
            token = &self.profile_token,
            pan = pan,
            tilt = tilt
        );

        let request = build_soap_envelope(&self.camera_config, body);

        let res = self
            .client
            .post(&self.ptz_service_url)
            .header(CONTENT_TYPE, "application/soap+xml")
            .body(request)
            .send()
            .await?
            .text()
            .await?;

        check_soap_fault(&res)
    }

    pub async fn ptz_stop(&self) -> anyhow::Result<()> {
        // See example request in Annex B, at B.5.3.2 in the
        // ONVIF Application Programmers Guide
        let body = format!(
            r#"
        <tptz:Stop xmlns:tptz="{NS_PTZ}">
            <tptz:ProfileToken>{token}</tptz:ProfileToken>
            <tptz:PanTilt>true</tptz:PanTilt>
            <tptz:Zoom>true</tptz:Zoom>
        </tptz:Stop>
        "#,
            token = &self.profile_token
        );

        let request = build_soap_envelope(&self.camera_config, body);

        let res = self
            .client
            .post(&self.ptz_service_url)
            .header(CONTENT_TYPE, "application/soap+xml")
            .body(request)
            .send()
            .await?
            .text()
            .await?;

        check_soap_fault(&res)
    }
}

async fn get_service_urls(
    client: &reqwest::Client,
    camera_config: &CameraConfig,
) -> anyhow::Result<(String, String)> {
    // The device (management) service is at /onvif/device_service. See section 5.1.1 in
    // https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf
    let device_service_url = Url::parse(&format!(
        "http://{ip}:{onvif_port}/onvif/device_service",
        ip = &camera_config.ip,
        onvif_port = camera_config.onvif_port()
    ))?;

    let body = format!(
        r#"
            <tds:GetCapabilities xmlns:tds="{NS_DEVICE}">
                <tds:Category>All</tds:Category>
            </tds:GetCapabilities>
            "#
    );
    let request = build_soap_envelope(camera_config, body);

    let res = client
        .post(device_service_url)
        .header(CONTENT_TYPE, "application/soap+xml")
        .body(request)
        .send()
        .await?
        .text()
        .await?;

    let doc = roxmltree::Document::parse(&res)?;

    let media_service_url = find_xaddr(&doc, "Media")?;
    let ptz_service_url = find_xaddr(&doc, "PTZ")?;

    Ok((media_service_url, ptz_service_url))
}

async fn get_profile_token(
    client: &reqwest::Client,
    media_service_url: &str,
    camera_config: &CameraConfig,
) -> anyhow::Result<String> {
    let body = format!(
        r#"
        <trt:GetProfiles xmlns:trt="{NS_MEDIA}"/>
        "#
    );
    let request = build_soap_envelope(camera_config, body);
    let res = client
        .post(media_service_url)
        .header(CONTENT_TYPE, "application/soap+xml")
        .body(request)
        .send()
        .await?
        .text()
        .await?;

    let doc = roxmltree::Document::parse(&res)?;
    find_profile_token(&doc)
}

fn build_soap_envelope(camera_config: &CameraConfig, body: String) -> String {
    format!(
        r#"
            <s:Envelope xmlns:s="{NS_SOAP}">
                <s:Header>{auth_header}</s:Header>
                <s:Body>
                {body}
                </s:Body>
            </s:Envelope>
            "#,
        auth_header = build_auth_header(camera_config),
    )
}
fn build_auth_header(camera_config: &CameraConfig) -> String {
    // See https://www.onvif.org/wp-content/uploads/2016/12/ONVIF_WG-APG-Application_Programmers_Guide-1.pdf
    // section 6.1 for info on generating the digest.
    let raw_nonce: [u8; 16] = rand::rng().random();
    let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut hasher = Sha1::new();
    hasher.update(raw_nonce);
    hasher.update(created_at.as_bytes());
    hasher.update(camera_config.password.as_bytes());

    let digest = BASE64_STANDARD.encode(hasher.finalize());

    // We need the nonce Base64-encoded separately for the header's wsse:Nonce child
    let nonce_base64 = BASE64_STANDARD.encode(raw_nonce);

    // Note: XML ignores whitespaces between elements, so we can
    // indent the string to make it more readable
    format!(
        r#"
            <wsse:Security
              xmlns:wsse="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd"
              xmlns:wsu="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">
              <wsse:UsernameToken>
                <wsse:Username>{username}</wsse:Username>
                <wsse:Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">{digest}</wsse:Password>
                <wsse:Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{nonce}</wsse:Nonce>
                <wsu:Created>{created}</wsu:Created>
              </wsse:UsernameToken>
            </wsse:Security>
            "#,
        username = camera_config.user,
        digest = digest,
        nonce = nonce_base64,
        created = created_at,
    )
}

fn find_xaddr(doc: &roxmltree::Document, service: &str) -> anyhow::Result<String> {
    // According to ONVIF, `XAddr` elements represent service addresses.
    // See section 8.1.2.1 in ONVIF-Core-Specification
    //
    doc.descendants()
        .find(|n| n.tag_name().name() == service && n.tag_name().namespace() == Some(NS_SCHEMA))
        .and_then(|n| {
            n.children().find(|c| {
                c.tag_name().name() == "XAddr" && c.tag_name().namespace() == Some(NS_SCHEMA)
            })
        })
        .and_then(|n| n.text())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No XAddr found for service: {service}"))
}

fn check_soap_fault(response: &str) -> anyhow::Result<()> {
    let doc = roxmltree::Document::parse(response)?;
    if let Some(fault) = doc.descendants().find(|n| n.tag_name().name() == "Fault") {
        let reason = fault
            .descendants()
            .find(|n| n.tag_name().name() == "Text")
            .and_then(|n| n.text())
            .unwrap_or("unknown error");
        return Err(anyhow::anyhow!("SOAP fault: {reason}"));
    }
    Ok(())
}

fn find_profile_token(doc: &roxmltree::Document) -> anyhow::Result<String> {
    // According to ONVIF, the `Profiles` element will contain a `token` attribute.
    // See Annex B.4.1.1 in ONVIF Application Programmers Guide
    //
    doc.descendants()
        .find(|n| n.tag_name().name() == "Profiles" && n.tag_name().namespace() == Some(NS_MEDIA))
        .and_then(|n| n.attribute("token"))
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No profile token found"))
}
