use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl TryFrom<String> for Version {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("invalid version '{}', expected 'X.Y.Z'", s));
        }
        Ok(Self {
            major: parts[0].parse().map_err(|_| "invalid major")?,
            minor: parts[1].parse().map_err(|_| "invalid minor")?,
            patch: parts[2].parse().map_err(|_| "invalid patch")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_default() {
        let v = Version::default();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_display() {
        assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
        assert_eq!(Version::new(0, 1, 0).to_string(), "0.1.0");
        assert_eq!(Version::default().to_string(), "0.0.0");
    }

    #[test]
    fn test_from_str() {
        assert_eq!("1.2.3".parse::<Version>().unwrap(), Version::new(1, 2, 3));
        assert_eq!("0.1.0".parse::<Version>().unwrap(), Version::new(0, 1, 0));
        assert_eq!(
            "10.20.30".parse::<Version>().unwrap(),
            Version::new(10, 20, 30)
        );
    }

    #[test]
    fn test_from_str_invalid() {
        assert!("1.2".parse::<Version>().is_err());
        assert!("1.2.3.4".parse::<Version>().is_err());
        assert!("a.b.c".parse::<Version>().is_err());
        assert!("1.2.x".parse::<Version>().is_err());
        assert!("".parse::<Version>().is_err());
    }

    #[test]
    fn test_equality() {
        assert_eq!(Version::new(1, 0, 0), Version::new(1, 0, 0));
        assert_ne!(Version::new(1, 0, 0), Version::new(2, 0, 0));
    }

    #[test]
    fn test_clone() {
        let v1 = Version::new(1, 2, 3);
        let v2 = v1.clone();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_serialize() {
        #[derive(Serialize)]
        struct Config {
            version: Version,
        }
        let config = Config {
            version: Version::new(1, 2, 3),
        };
        let toml = toml::to_string(&config).unwrap();
        assert_eq!(toml.trim(), r#"version = "1.2.3""#);
    }

    #[test]
    fn test_deserialize() {
        #[derive(Deserialize)]
        struct Config {
            version: Version,
        }
        let config: Config = toml::from_str(r#"version = "1.2.3""#).unwrap();
        assert_eq!(config.version, Version::new(1, 2, 3));
    }
}
