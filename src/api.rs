#[derive(Clone)]
pub struct SecurityConfig {
    /// Host (no scheme/port) the service is reached at on the LAN.
    pub allowed_host: String,
}

impl SecurityConfig {
    pub fn host_ok(&self, host_header: Option<&str>) -> bool {
        match host_header {
            Some(h) => h.split(':').next() == Some(self.allowed_host.as_str()),
            None => false,
        }
    }
    /// Origin is only sent by browsers. Absent = non-browser client (curl) = allow.
    /// Present = must match the expected host (defeats DNS-rebinding/CSRF).
    pub fn origin_ok(&self, origin_header: Option<&str>) -> bool {
        match origin_header {
            None => true,
            Some(o) => o
                .strip_prefix("http://")
                .or_else(|| o.strip_prefix("https://"))
                .map(|rest| rest.split(':').next() == Some(self.allowed_host.as_str()))
                .unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_lan_host() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.1.230:80")));
    }

    #[test]
    fn rejects_foreign_host_and_missing() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.host_ok(None));
    }

    #[test]
    fn origin_ok_only_for_expected_or_absent_nonbrowser() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(cfg.origin_ok(Some("http://192.168.1.230")));
        assert!(cfg.origin_ok(None));
        assert!(!cfg.origin_ok(Some("http://evil.example.com")));
    }
}
