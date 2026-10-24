//! WebDAV over blocking HTTP for Nextcloud Files, plus a tiny XML tree.

use crate::{Error, Result};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

pub const DAV: &str = "DAV:";
pub const OC: &str = "http://owncloud.org/ns";
pub const NC: &str = "http://nextcloud.org/ns";

/// Opening a connection (TLS included) gets this long: the system resends a dropped connection
/// request after 1 and 3 s, later ones are seldom answered. Then it is tried anew, up to
/// `CONNECT_TRIES` more times.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);
const CONNECT_TRIES: u32 = 3;

#[derive(Debug, Default, Clone)]
pub struct Element {
    pub ns: String,
    pub name: String,
    pub text: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn parse(xml: &str) -> Result<Element> {
        let mut r = quick_xml::NsReader::from_str(xml);
        let mut stack = vec![Element::default()];
        loop {
            let (ns, ev) = r
                .read_resolved_event()
                .map_err(|e| Error::Parse(e.to_string()))?;
            let ns = match ns {
                ResolveResult::Bound(n) => n.as_ref().to_string(),
                _ => String::new(),
            };
            match ev {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let attrs = e
                        .attributes()
                        .flatten()
                        .map(|a| {
                            let v = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map(|v| v.into_owned())
                                .unwrap_or_default();
                            (a.key.local_name().as_ref().to_string(), v)
                        })
                        .collect();
                    let el = Element {
                        ns,
                        name: e.local_name().as_ref().to_string(),
                        attrs,
                        ..Default::default()
                    };
                    stack.push(el);
                    if matches!(ev, Event::Empty(_)) {
                        let el = stack.pop().unwrap();
                        stack.last_mut().unwrap().children.push(el);
                    }
                }
                Event::End(_) => {
                    let el = stack.pop().unwrap();
                    stack
                        .last_mut()
                        .ok_or_else(|| Error::Parse("unbalanced xml".into()))?
                        .children
                        .push(el);
                }
                Event::Text(t) => stack.last_mut().unwrap().text += &*t,
                Event::CData(t) => stack.last_mut().unwrap().text += &*t,
                Event::GeneralRef(g) => {
                    let ent = format!("&{};", &*g);
                    let s = quick_xml::escape::unescape(&ent)
                        .map_err(|e| Error::Parse(e.to_string()))?;
                    stack.last_mut().unwrap().text += &s;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        stack
            .pop()
            .and_then(|mut root| root.children.pop())
            .ok_or_else(|| Error::Parse("empty xml".into()))
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, ns: &str, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.ns == ns && c.name == name)
    }

    pub fn children<'a>(&'a self, ns: &'a str, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children
            .iter()
            .filter(move |c| c.ns == ns && c.name == name)
    }

    /// Depth-first search for the first descendant with this name.
    pub fn find(&self, ns: &str, name: &str) -> Option<&Element> {
        self.children.iter().find_map(|c| {
            if c.ns == ns && c.name == name {
                Some(c)
            } else {
                c.find(ns, name)
            }
        })
    }

    pub fn find_text(&self, ns: &str, name: &str) -> Option<String> {
        self.find(ns, name)
            .map(|e| e.text.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

/// One `<d:response>` of a multistatus: href, HTTP status, and the merged 200 `<d:prop>`.
#[derive(Debug)]
pub struct DavResponse {
    pub href: String,
    pub status: u16,
    pub props: Element,
}

pub fn multistatus(xml: &str) -> Result<(Vec<DavResponse>, Option<String>)> {
    let root = Element::parse(xml)?;
    let status_code = |e: &Element| {
        e.child(DAV, "status")
            .and_then(|s| s.text.split_whitespace().nth(1)?.parse().ok())
    };
    let mut out = Vec::new();
    for r in root.children(DAV, "response") {
        let href = r
            .child(DAV, "href")
            .map(|h| h.text.trim().to_string())
            .unwrap_or_default();
        let mut props = Element::default();
        for ps in r.children(DAV, "propstat") {
            if status_code(ps) == Some(200)
                && let Some(p) = ps.children.iter().find(|c| c.name == "prop")
            {
                props.children.extend(p.children.iter().cloned());
            }
        }
        let status = status_code(r).unwrap_or(200);
        out.push(DavResponse {
            href: percent_decode(&href),
            status,
            props,
        });
    }
    let token = root
        .child(DAV, "sync-token")
        .map(|t| t.text.trim().to_string());
    Ok((out, token))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode_path(s: &str) -> String {
    let mut out = String::new();
    for &c in s.as_bytes() {
        if c.is_ascii_alphanumeric() || b"/-._~@!$&'()*+,;=:".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

pub struct HttpResponse {
    pub status: u16,
    pub etag: Option<String>,
    pub location: Option<String>,
    pub body: String,
}

pub struct Dav {
    agent: ureq::Agent,
    origin: String,
    root: String,
    auth: String,
}

impl Dav {
    /// `url` is the Nextcloud base (https://cloud.example) or a full DAV root.
    pub fn new(url: &str, user: &str, password: &str) -> Dav {
        let url = url.trim_end_matches('/');
        let root = if url.contains("/remote.php/") {
            format!("{url}/")
        } else {
            format!("{url}/remote.php/dav/")
        };
        let origin = url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
        let agent = ureq::Agent::config_builder()
            .allow_non_standard_methods(true)
            .user_agent("Notav")
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .build()
            .new_agent();
        Dav {
            agent,
            origin,
            root,
            auth: format!(
                "Basic {}",
                crate::b64encode(format!("{user}:{password}").as_bytes(), false)
            ),
        }
    }

    pub fn url(&self, href: &str) -> String {
        if href.starts_with("http") {
            href.to_string()
        } else {
            format!("{}{}", self.origin, percent_encode_path(href))
        }
    }

    pub fn request(
        &self,
        method: &str,
        href: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<HttpResponse> {
        let req = || {
            let mut b = ureq::http::Request::builder()
                .method(method)
                .uri(self.url(href))
                .header("Authorization", &self.auth);
            if !body.is_empty()
                && !headers
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            {
                b = b.header("Content-Type", "application/xml; charset=utf-8");
            }
            for (k, v) in headers {
                // Also one stored before ETags were made strong (below).
                let v = if k.eq_ignore_ascii_case("if-match") {
                    strong(v)
                } else {
                    v
                };
                b = b.header(*k, v);
            }
            b.body(body.to_string())
                .map_err(|e| Error::Net(e.to_string()))
        };
        // A connection the server never answers is tried again rather than waited out (until
        // the global timeout): nothing was sent on it yet.
        let mut tries = 0;
        let mut resp = loop {
            match self.agent.run(req()?) {
                Err(ureq::Error::Timeout(ureq::Timeout::Connect)) if tries < CONNECT_TRIES => {
                    tries += 1
                }
                r => break r.map_err(|e| Error::Net(e.to_string()))?,
            }
        };
        let header = |n: &str| {
            resp.headers()
                .get(n)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let etag = header("etag").map(|e| strong(&e).to_string());
        let location = header("location");
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Net(e.to_string()))?;
        if status == 401 {
            return Err(Error::Auth);
        }
        Ok(HttpResponse {
            status,
            etag,
            location,
            body,
        })
    }

    fn propfind(&self, href: &str, depth: &str, props: &str) -> Result<Vec<DavResponse>> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:oc="{OC}" xmlns:nc="{NC}"><d:prop>{props}</d:prop></d:propfind>"#
        );
        let r = self.request("PROPFIND", href, &[("Depth", depth)], &body)?;
        if r.status != 207 {
            return Err(Error::Http(r.status, r.body));
        }
        Ok(multistatus(&r.body)?.0)
    }

    pub fn principal(&self) -> Result<String> {
        let p = self.propfind(&self.root, "0", "<d:current-user-principal/>")?;
        p.first()
            .and_then(|r| r.props.find_text(DAV, "href"))
            .ok_or_else(|| Error::Parse("no current-user-principal".into()))
    }

    /// GET a resource: (etag, body), or None if gone.
    pub fn get(&self, href: &str) -> Result<Option<(Option<String>, String)>> {
        let r = self.request("GET", href, &[], "")?;
        match r.status {
            200 => Ok(Some((r.etag, r.body))),
            404 => Ok(None),
            s => Err(Error::Http(s, r.body)),
        }
    }
}

/// A proxy that compresses a response marks its ETag weak ("W/"), and If-Match never matches a
/// weak one (412 every time); the value under it is the resource's own, strong ETag.
fn strong(etag: &str) -> &str {
    etag.strip_prefix("W/").unwrap_or(etag)
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multistatus() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
 <d:response><d:href>/remote.php/dav/files/Test/Notes/Zakupy%20%26%20dom.md</d:href>
  <d:propstat><d:prop><d:getetag>&quot;6f1e&quot;</d:getetag><oc:fileid>42</oc:fileid>
   <d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><d:getcontenttype/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
 <d:response><d:href>/x/gone.md</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
</d:multistatus>"#;
        let (r, _) = multistatus(xml).unwrap();
        assert_eq!(
            r[0].href,
            "/remote.php/dav/files/Test/Notes/Zakupy & dom.md"
        );
        assert_eq!(r[0].props.find_text(DAV, "getetag").unwrap(), "\"6f1e\"");
        assert_eq!(r[0].props.find_text(OC, "fileid").unwrap(), "42");
        assert!(r[0].props.child(DAV, "getcontenttype").is_none());
        assert_eq!((r[1].href.as_str(), r[1].status), ("/x/gone.md", 404));
    }

    #[test]
    fn helpers() {
        assert_eq!(crate::b64encode(b"Test:6*U", false), "VGVzdDo2KlU=");
        assert_eq!(crate::b64encode(b"ab", false), "YWI=");
        assert_eq!(percent_encode_path("/a b/ż.md"), "/a%20b/%C5%BC.md");
        assert_eq!(percent_decode("/a%20b/%C5%BC.md"), "/a b/ż.md");
    }
}
