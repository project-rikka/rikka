use crate::http::Http;
use crate::stdio::Stdio;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    Stdio(Stdio),
    Http(Http),
}

impl Transport {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Transport::Stdio(_) => "stdio",
            Transport::Http(_) => "http",
        }
    }
}

impl std::fmt::Display for Transport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transport::Stdio(stdio) => formatter.write_str(&stdio.command_line()),
            Transport::Http(http) => formatter.write_str(http.url()),
        }
    }
}

impl From<Stdio> for Transport {
    fn from(stdio: Stdio) -> Transport {
        Transport::Stdio(stdio)
    }
}

impl From<Http> for Transport {
    fn from(http: Http) -> Transport {
        Transport::Http(http)
    }
}
