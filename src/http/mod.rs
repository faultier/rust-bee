//! HTTP parser.

#![experimental]

use std::char::to_lowercase;
use std::fmt::{Formatter, FormatError, Show};
use std::io::{IoError, IoResult};
use UINT_MAX = std::uint::MAX;

#[deriving(PartialEq, Eq, Clone, Show)]
/// A parser types.
pub enum Type {
    /// Parse request.
    Request,
    /// Parse response.
    Response,
    /// Parse request or response.
    Both,
}

/// A list of supported HTTP versions.
#[allow(non_camel_case_types)]
#[deriving(PartialEq, Eq, Clone)]
pub enum HttpVersion {
    /// HTTP/0.9
    HTTP_0_9,
    /// HTTP/1.0
    HTTP_1_0,
    /// HTTP/1.1
    HTTP_1_1,
}

impl HttpVersion {
    /// Detect HTTP version with major and minor.
    pub fn find(major: uint, minor: uint) -> Option<HttpVersion> {
        match major {
            0 if minor == 9 => Some(HTTP_0_9),
            1 => match minor {
                0 => Some(HTTP_1_0),
                1 => Some(HTTP_1_1),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Show for HttpVersion {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        match *self {
            HTTP_0_9 => f.pad("HTTP/0.9"),
            HTTP_1_0 => f.pad("HTTP/1.0"),
            HTTP_1_1 => f.pad("HTTP/1.1"),
        }
    }
}

#[allow(missing_doc)]
#[deriving(PartialEq, Eq, Clone)]
pub enum HttpMethod {
    HttpCheckout,
    HttpConnect,
    HttpCopy,
    HttpDelete,
    HttpGet,
    HttpHead,
    HttpLink,
    HttpLock,
    HttpMerge,
    HttpMkActivity,
    HttpMkCalendar,
    HttpMkCol,
    HttpMove,
    HttpMsearch,
    HttpNotify,
    HttpOptions,
    HttpPatch,
    HttpPost,
    HttpPropFind,
    HttpPropPatch,
    HttpPurge,
    HttpPut,
    HttpReport,
    HttpSearch,
    HttpSubscribe,
    HttpTrace,
    HttpUnlink,
    HttpUnlock,
    HttpUnsubscribe,
}

impl HttpMethod {
    #[inline]
    fn name(&self) -> &'static str {
        match *self {
            HttpCheckout    => "CHECKOUT",
            HttpConnect     => "CONNECT",
            HttpCopy        => "COPY",
            HttpDelete      => "DELETE",
            HttpGet         => "GET",
            HttpHead        => "HEAD",
            HttpLink        => "LINK",
            HttpLock        => "LOCK",
            HttpMerge       => "MERGE",
            HttpMkActivity  => "MKACTIVITY",
            HttpMkCalendar  => "MKCALENDAR",
            HttpMkCol       => "MKCOL",
            HttpMove        => "MOVE",
            HttpMsearch     => "M-SEARCH",
            HttpNotify      => "NOTIFY",
            HttpOptions     => "OPTIONS",
            HttpPatch       => "PATCH",
            HttpPost        => "POST",
            HttpPropFind    => "PROPFIND",
            HttpPropPatch   => "PROPPATCH",
            HttpPut         => "PUT",
            HttpPurge       => "PURGE",
            HttpReport      => "REPORT",
            HttpSearch      => "SEARCH",
            HttpSubscribe   => "SUBSCRIBE",
            HttpTrace       => "TRACE",
            HttpUnlink      => "UNLINK",
            HttpUnlock      => "UNLOCK",
            HttpUnsubscribe => "UNSUBSCRIBE",
        }
    }

    #[inline]
    fn hit(&self, pos: uint, c: char) -> bool {
        self.name().char_at(pos) == c
    }
}

impl Show for HttpMethod {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        f.pad(self.name())
    }
}

/// Parser event handler.
pub trait Handler {
    #[allow(unused_variable)]
    /// Called when start to parsing of message.
    /// Default implementation is nothing to do.
    fn on_message_begin(&mut self, parser: &Parser) {
    }

    #[allow(unused_variable)]
    /// Called when url parsed.
    /// Default implementation is nothing to do.
    fn on_url(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when header field's name parsed.
    /// Default implementation is nothing to do.
    fn on_header_field(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when header field's value parsed.
    /// Default implementation is nothing to do.
    fn on_header_value(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of headers.
    /// Default implementation is nothing to do.
    fn on_headers_complete(&mut self, parser: &Parser) -> bool{
        return false;
    }

    #[allow(unused_variable)]
    /// Called when body parsed.
    /// Default implementation is nothing to do.
    fn on_body(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of whole message.
    /// Default implementation is nothing to do.
    fn on_message_complete(&mut self, parser: &Parser) {
    }

    /// Push partial data, e.g. URL, header field, message body.
    fn push_data(&mut self, &Parser, u8);

    /// Push partial data, e.g. URL, header field, message body.
    fn push_data_all(&mut self, parser: &Parser, data: &[u8]) {
        for &byte in data.iter() {
            self.push_data(parser, byte);
        }
    }
}

/// A list specifying categories of parse errors.
#[deriving(PartialEq, Eq, Clone, Show)]
pub enum ParseError {
    /// Any parse error not part of this list.
    OtherParseError,
    /// Invalid HTTP method.
    InvalidMethod,
    /// Invalid URL.
    InvalidUrl,
    /// Invalid HTTP version.
    InvalidVersion,
    /// Invalid request line.
    InvalidRequestLine,
    /// Invalid status code.
    InvalidStatusCode,
    /// Invalid status line.
    InvalidStatusLine,
    /// Invalid header field.
    InvalidHeaderField,
    /// Invalid header section.
    InvalidHeaders,
    /// Expected data, but reached EOF.
    InvalidEOFState,
    /// An I/O error occurred.
    AnyIoError(IoError),
}

pub type ParseResult = Result<uint, ParseError>;

static CR: char = '\r';
static LF: char = '\n';

macro_rules! reset_state (
    ($t:expr) => (match $t {
        Request  => StartReq,
        Response => StartRes,
        Both     => StartReqOrRes,
    })
)

#[allow(dead_code)]
/// HTTP parser.
pub struct Parser {
    // parser internal state
    parser_type: Type,
    state: ParserState,
    hstate: HeaderParseState,
    index: uint,
    skip_body: bool,

    // http version
    http_version: Option<HttpVersion>,
    major: uint,
    minor: uint,

    // common header
    content_length: uint,
    upgrade: bool,

    // request
    method: Option<HttpMethod>,
    keep_alive: bool,

    // response
    status_code: uint,
}

impl Parser {
    /// Create a new `Parser`.
    pub fn new(t: Type) -> Parser {
        Parser {
            parser_type: t,
            http_version: None,
            state: reset_state!(t),
            hstate: HeaderGeneral,
            method: None,
            status_code: 0,
            content_length: UINT_MAX,
            skip_body: false,
            index: 0,
            major: 0,
            minor: 0,
            keep_alive: false,
            upgrade: false,
        }
    }

    #[allow(unused_must_use)]
    /// Parse HTTP message.
    pub fn parse<C: Handler>(&mut self, data: &[u8], handler: &mut C) -> ParseResult {
        if self.state == Dead { return Ok(0) }
        if self.state == Crashed { return Err(OtherParseError) }
        if data.len() == 0 { return Ok(0) }

        let mut read = 0u;

        if !(self.state == BodyIdentity || self.state == BodyIdentityEOF) {
            for &byte in data.iter() {
                read += 1;
                match self.state {
                    StartReq => {
                        self.major = 0;
                        self.minor = 0;
                        self.http_version = None;
                        self.content_length = UINT_MAX;
                        self.skip_body = false;
                        self.method = Some(match byte as char {
                            'C' => HttpConnect,     // or CHECKOUT, COPY
                            'D' => HttpDelete,
                            'G' => HttpGet,
                            'H' => HttpHead,
                            'L' => HttpLink,        // or LOCK
                            'M' => HttpMkCol,       // or M-SEARCH, MERGE, MKACTIVITY, MKCALENDER
                            'N' => HttpNotify,
                            'O' => HttpOptions,
                            'P' => HttpPut,         // or PATCH, POST, PROPPATCH, PROPFIND
                            'R' => HttpReport,
                            'S' => HttpSearch,      // or SUBSCRIBE
                            'T' => HttpTrace,
                            'U' => HttpUnlink,      // or UNLOCK, UNSUBSCRIBE
                            CR | LF => break,
                            _   => { self.state = Crashed; return Err(InvalidMethod) },
                        });
                        handler.on_message_begin(self);
                        self.state = ReqMethod;
                        self.index = 1;
                    }
                    StartRes => {
                        self.major = 0;
                        self.minor = 0;
                        self.status_code = 0;
                        self.http_version = None;
                        self.content_length = UINT_MAX;
                        self.skip_body = false;
                        match byte as char {
                            'H' => {
                                self.state = ResHttpStart;
                                self.index = 1;
                            },
                            CR | LF => break,
                            _   => { self.state = Crashed; return Err(InvalidMethod) },
                        }
                        handler.on_message_begin(self);
                    }
                    ReqMethod => {
                        let method = self.method.unwrap();
                        if byte as char == ' ' {
                            self.state = ReqUrl;
                            self.index = 0;
                        } else {
                            if !method.hit(self.index, byte as char) {
                                self.method = Some(match method {
                                    HttpConnect    if self.index == 2 && byte as char == 'H' => HttpCheckout,
                                    HttpConnect    if self.index == 3 && byte as char == 'P' => HttpCheckout,
                                    HttpLink       if self.index == 1 && byte as char == 'O' => HttpLock,
                                    HttpMkCol      if self.index == 1 && byte as char == '-' => HttpMsearch,
                                    HttpMkCol      if self.index == 1 && byte as char == 'E' => HttpMerge,
                                    HttpMkCol      if self.index == 2 && byte as char == 'A' => HttpMkActivity,
                                    HttpMkCol      if self.index == 3 && byte as char == 'A' => HttpMkCalendar,
                                    HttpPut        if self.index == 1 && byte as char == 'A' => HttpPatch,
                                    HttpPut        if self.index == 1 && byte as char == 'O' => HttpPost,
                                    HttpPut        if self.index == 1 && byte as char == 'R' => HttpPropPatch,
                                    HttpPut        if self.index == 2 && byte as char == 'R' => HttpPurge,
                                    HttpPropPatch  if self.index == 4 && byte as char == 'F' => HttpPropFind,
                                    HttpSearch     if self.index == 1 && byte as char == 'U' => HttpSubscribe,
                                    HttpUnlink     if self.index == 2 && byte as char == 'S' => HttpUnsubscribe,
                                    HttpUnlink     if self.index == 3 && byte as char == 'O' => HttpUnlock,
                                    _ => { self.state = Crashed; return Err(InvalidMethod) },
                                });
                            }
                            self.index += 1;
                        }
                    }
                    ReqUrl => {
                        match byte as char {
                            ' ' => {
                                if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                                match handler.on_url(self, self.index) {
                                    Ok(()) => {
                                        self.state = ReqHttpStart;
                                        self.index = 0;
                                    }
                                    Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                }
                            }
                            CR | LF => {
                                if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                                self.http_version = Some(HTTP_0_9);
                                match handler.on_url(self, self.index) {
                                    Ok(()) => {
                                        self.state = Dead;
                                        self.index = 0;
                                        handler.on_message_complete(self);
                                        break;
                                    }
                                    Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                }
                            }
                            _ => {
                                handler.push_data(self, byte);
                                self.index += 1;
                            }
                        }
                    }
                    ReqHttpStart => {
                        let c = byte as char;
                        if (c != 'H' && self.index == 0)
                            || (c != 'T' && (self.index == 1 || self.index == 2))
                            || (c != 'P' && self.index == 3)
                            || (c != '/' && self.index == 4)
                            || ((byte < '0' as u8 || byte > '9' as u8) && self.index == 5) {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                        if self.index == 5 {
                            self.state = ReqHttpMajor;
                            self.major = byte as uint - '0' as uint;
                            self.index = 1;
                        } else {
                            self.index += 1;
                        }
                    }
                    ReqHttpMajor => {
                        match byte as char {
                            '.' if self.index > 0 => {
                                self.state = ReqHttpMinor;
                                self.index = 0;
                            }
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.major *= 10;
                                self.major += n as uint - '0' as uint;
                            }
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ReqHttpMinor => {
                        match byte as char {
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.minor *= 10;
                                self.minor += n as uint - '0' as uint;
                            }
                            CR | LF if self.index > 0 => match HttpVersion::find(self.major, self.minor) {
                                None => { self.state = Crashed; return Err(InvalidVersion) },
                                v => {
                                    self.http_version = v;
                                    self.keep_alive = v == Some(HTTP_1_1);
                                    self.state = if byte as char == CR {
                                        ReqLineAlmostDone
                                    } else {
                                        HeaderFieldStart
                                    };
                                    self.index = 0;
                                }
                            },
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ReqLineAlmostDone => {
                        if byte as char != LF {
                            return Err(InvalidRequestLine);
                        }
                        self.state = HeaderFieldStart;
                    }
                    ResHttpStart => {
                        let c = byte as char;
                        if (c != 'T' && (self.index == 1 || self.index == 2))
                            || (c != 'P' && self.index == 3)
                            || (c != '/' && self.index == 4)
                            || ((byte < '0' as u8 || byte > '9' as u8) && self.index == 5) {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                        if self.index == 5 {
                            self.state = ResHttpMajor;
                            self.major = byte as uint - '0' as uint;
                            self.index = 1;
                        } else {
                            self.index += 1;
                        }
                    }
                    ResHttpMajor => {
                        match byte as char {
                            '.' if self.index > 0 => {
                                self.state = ResHttpMinor;
                                self.index = 0;
                            }
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.major *= 10;
                                self.major += n as uint - '0' as uint;
                            }
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ResHttpMinor => {
                        match byte as char {
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.minor *= 10;
                                self.minor += n as uint - '0' as uint;
                            }
                            ' ' if self.index > 0 => match HttpVersion::find(self.major, self.minor) {
                                None => { self.state = Crashed; return Err(InvalidVersion) },
                                v => {
                                    self.http_version = v;
                                    self.keep_alive = v == Some(HTTP_1_1);
                                    self.state = ResStatusCode;
                                    self.index = 0;
                                }
                            },
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ResStatusCodeStart => {
                        if byte >= '0' as u8 && byte <= '9' as u8 {
                            self.state = ResStatusCode;
                            self.status_code = byte as uint - '0' as uint;
                            self.index = 1;
                        } else if byte as char != ' ' {
                            self.state = Crashed;
                            return Err(InvalidStatusCode);
                        }
                    }
                    ResStatusCode => {
                        if byte >= '0' as u8 && byte <= '9' as u8 && self.index < 3 {
                            self.status_code *= 10;
                            self.status_code += byte as uint - '0' as uint;
                            self.index += 1;
                        } else {
                            self.state = match byte as char {
                                ' ' => ResStatus,
                                CR  => ResLineAlmostDone,
                                LF  => HeaderFieldStart,
                                _   => {
                                    self.state = Crashed;
                                    return Err(InvalidStatusLine);
                                }
                            };
                            self.index = 0;
                        }
                    }
                    ResStatus => {
                        self.state = match byte as char {
                            CR => ResLineAlmostDone,
                            LF => HeaderFieldStart,
                            _  => ResStatus,
                        };
                    }
                    ResLineAlmostDone => {
                        if byte as char != LF {
                            return Err(InvalidStatusLine);
                        }
                        self.state = HeaderFieldStart;
                    }
                    HeaderFieldStart => {
                        match byte as char {
                            CR => self.state = HeadersAlmostDone,
                            LF => {
                                self.state = if handler.on_headers_complete(self) || self.skip_body {
                                    handler.on_message_complete(self);
                                    reset_state!(self.parser_type)
                                } else {
                                    match self.content_length {
                                        0u => {
                                            handler.on_message_complete(self);
                                            reset_state!(self.parser_type)
                                        }
                                        UINT_MAX => if self.parser_type == Request || !self.needs_eof() {
                                            handler.on_message_complete(self);
                                            reset_state!(self.parser_type)
                                        } else {
                                            BodyIdentityEOF
                                        },
                                        _ => BodyIdentity,
                                    }
                                };
                                break
                            }
                            c if is_token(c) => {
                                self.state = HeaderField;
                                self.hstate = match to_lowercase(c) {
                                    'c' => HeaderConnection,
                                    't' => HeaderTransferEncoding,
                                    'u' => HeaderUpgrade,
                                    _   => HeaderGeneral,
                                };
                                handler.push_data(self, byte);
                                self.index = 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                    HeaderField => {
                        match byte as char {
                            ':' => {
                                match handler.on_header_field(self, self.index) {
                                    Ok(()) => {
                                        self.state = HeaderValueDiscardWS;
                                        self.index = 0;
                                    },
                                    Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                }
                            }
                            CR => {
                                self.state = HeaderAlmostDone;
                                self.index = 0;
                            }
                            LF => {
                                self.state = HeaderFieldStart;
                                self.index = 0;
                            }
                            c if is_token(c) => {
                                if self.hstate != HeaderGeneral {
                                    self.hstate = match self.hstate {
                                        HeaderConnection => match to_lowercase(c) {
                                            'o' if self.index == 1 => HeaderConnection,
                                            'n' if self.index == 2 => HeaderConnection,
                                            'n' if self.index == 3 => HeaderConnection,
                                            'e' if self.index == 4 => HeaderConnection,
                                            'c' if self.index == 5 => HeaderConnection,
                                            't' if self.index == 6 => HeaderConnection,
                                            'i' if self.index == 7 => HeaderConnection,
                                            'o' if self.index == 8 => HeaderConnection,
                                            'n' if self.index == 9 => HeaderConnection,
                                            't' if self.index == 3 => HeaderContentLength,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderContentLength => match to_lowercase(c) {
                                            'e' if self.index == 4  => HeaderContentLength,
                                            'n' if self.index == 5  => HeaderContentLength,
                                            't' if self.index == 6  => HeaderContentLength,
                                            '-' if self.index == 7  => HeaderContentLength,
                                            'l' if self.index == 8  => HeaderContentLength,
                                            'e' if self.index == 9  => HeaderContentLength,
                                            'n' if self.index == 10 => HeaderContentLength,
                                            'g' if self.index == 11 => HeaderContentLength,
                                            't' if self.index == 12 => HeaderContentLength,
                                            'h' if self.index == 13 => HeaderContentLength,
                                            _ => HeaderGeneral,
                                        },
                                        _ => HeaderGeneral,
                                    };
                                }
                                handler.push_data(self, byte);
                                self.index += 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                    HeaderValueDiscardWS => {
                        match byte as char {
                            ' ' | '\t' => (), // skip
                            CR => self.state = HeaderValueDiscardWSAlmostDone,
                            LF => self.state = HeaderValueDiscardLWS,
                            _ => {
                                let c = to_lowercase(byte as char);
                                self.hstate = match self.hstate {
                                    HeaderConnection if c == 'k' => HeaderMatchingKeepAlive,
                                    HeaderConnection if c == 'c' => HeaderMatchingClose,
                                    HeaderConnection if c == 'u' => HeaderMatchingUpgrade,
                                    HeaderContentLength => {
                                        self.content_length = byte as uint - '0' as uint;
                                        HeaderContentLength
                                    },
                                    _ => HeaderGeneral,
                                };
                                self.state = HeaderValue;
                                handler.push_data(self, byte);
                                self.index += 1;
                            },
                        }
                    }
                    HeaderValueDiscardWSAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderValueDiscardLWS;
                    }
                    HeaderValueDiscardLWS => {
                        if byte as char == ' ' || byte as char == '\t' {
                            self.state = HeaderValueDiscardWS;
                        } else {
                            // header value is empty.
                            match handler.on_header_value(self, 0) {
                                Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                _ => self.index = 0,
                            }
                            match byte as char {
                                CR => self.state = HeadersAlmostDone,
                                LF => {
                                    self.state = if handler.on_headers_complete(self) || self.skip_body {
                                        handler.on_message_complete(self);
                                        reset_state!(self.parser_type)
                                    } else {
                                        match self.content_length {
                                            0u => {
                                                handler.on_message_complete(self);
                                                reset_state!(self.parser_type)
                                            }
                                            UINT_MAX => if self.parser_type == Request || !self.needs_eof() {
                                                handler.on_message_complete(self);
                                                reset_state!(self.parser_type)
                                            } else {
                                                BodyIdentityEOF
                                            },
                                            _ => BodyIdentity,
                                        }
                                    };
                                    break
                                }
                                c if is_token(c) => {
                                    handler.push_data(self, byte);
                                    self.state = HeaderFieldStart;
                                    self.index = 1;
                                }
                                _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                            }
                        }
                    }
                    HeaderValue => {
                        match byte as char {
                            CR | LF => {
                                self.state = if byte as char == CR {
                                    HeaderAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                match self.hstate {
                                    HeaderMatchingKeepAlive if self.index == 10 => self.keep_alive = true,
                                    HeaderMatchingClose     if self.index == 5  => self.keep_alive = false,
                                    HeaderMatchingUpgrade   if self.index == 6  => self.upgrade = true,
                                    _ => (),
                                }
                                match handler.on_header_value(self, self.index) {
                                    Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                    _ => self.index = 0,
                                }
                            }
                            _ => {
                                if self.hstate != HeaderGeneral && is_token(byte as char) {
                                    let c = to_lowercase(byte as char);
                                    self.hstate = match self.hstate {
                                        HeaderMatchingKeepAlive => match c {
                                            'e' if self.index == 1 => HeaderMatchingKeepAlive,
                                            'e' if self.index == 2 => HeaderMatchingKeepAlive,
                                            'p' if self.index == 3 => HeaderMatchingKeepAlive,
                                            '-' if self.index == 4 => HeaderMatchingKeepAlive,
                                            'a' if self.index == 5 => HeaderMatchingKeepAlive,
                                            'l' if self.index == 6 => HeaderMatchingKeepAlive,
                                            'i' if self.index == 7 => HeaderMatchingKeepAlive,
                                            'v' if self.index == 8 => HeaderMatchingKeepAlive,
                                            'e' if self.index == 9 => HeaderMatchingKeepAlive,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderMatchingClose => match c {
                                            'l' if self.index == 1 => HeaderMatchingClose,
                                            'o' if self.index == 2 => HeaderMatchingClose,
                                            's' if self.index == 3 => HeaderMatchingClose,
                                            'e' if self.index == 4 => HeaderMatchingClose,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderMatchingUpgrade => match c {
                                            'p' if self.index == 1 => HeaderMatchingUpgrade,
                                            'g' if self.index == 2 => HeaderMatchingUpgrade,
                                            'r' if self.index == 3 => HeaderMatchingUpgrade,
                                            'a' if self.index == 4 => HeaderMatchingUpgrade,
                                            'd' if self.index == 5 => HeaderMatchingUpgrade,
                                            'e' if self.index == 6 => HeaderMatchingUpgrade,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderContentLength if byte >= '0' as u8 && byte <= '9' as u8 => {
                                            self.content_length *= 10;
                                            self.content_length += byte as uint - '0' as uint;
                                            HeaderContentLength
                                        }
                                        HeaderContentLength if byte < '0' as u8 || byte > '9' as u8 => {
                                            self.content_length = UINT_MAX;
                                            HeaderGeneral
                                        }
                                        _ => HeaderGeneral,
                                    };
                                }
                                handler.push_data(self, byte);
                                self.index += 1;
                            }
                        }
                    }
                    HeaderAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderFieldStart;
                    }
                    HeadersAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaders) }
                        self.state = if handler.on_headers_complete(self) || self.skip_body {
                            handler.on_message_complete(self);
                            reset_state!(self.parser_type)
                        } else {
                            match self.content_length {
                                0u => {
                                    handler.on_message_complete(self);
                                    reset_state!(self.parser_type)
                                }
                                UINT_MAX => if self.parser_type == Request || !self.needs_eof() {
                                    handler.on_message_complete(self);
                                    reset_state!(self.parser_type)
                                } else {
                                    BodyIdentityEOF
                                },
                                _ => BodyIdentity,
                            }
                        };
                        break
                    }
                    BodyIdentity | BodyIdentityEOF | Dead | Crashed => unreachable!(),
                    _ => unimplemented!()
                }
            }
        }

        match self.state {
            BodyIdentity => {
                let rest = data.len() - read;
                if rest >= self.content_length {
                    handler.push_data_all(self, data.slice(read, read + self.content_length));
                    handler.on_body(self, self.content_length);
                    handler.on_message_complete(self);
                    read += self.content_length;
                    self.state = reset_state!(self.parser_type);
                } else {
                    handler.push_data_all(self, data.slice_from(read));
                    read += rest;
                    self.content_length -= rest;
                }
            }
            _ => (), // unimplemented!(),
        }

        return Ok(read);
    }

    /// HTTP version
    pub fn get_http_version(&self) -> Option<HttpVersion> {
        self.http_version
    }

    /// HTTP satus code
    pub fn get_status_code(&self) -> uint {
        self.status_code
    }

    /// Connection: keep-alive or Connection: close
    pub fn should_keep_alive(&self) -> bool {
        self.keep_alive
    }

    /// Connection: upgrade
    pub fn should_upgrade(&self) -> bool {
        self.upgrade
    }

    fn needs_eof(&mut self) -> bool {
        if self.parser_type == Request {
            return false;
        }
        if self.status_code / 100 == 1 ||     // 1xx e.g. Continue
            self.status_code == 204 ||        // No Content
            self.status_code == 304 ||        // Not Modified
            self.skip_body {
            return false;
        }
        // TODO: chanked
        return true;
    }
}

#[inline]
fn is_token(c: char) -> bool {
    (c >= '^' && c <= 'z')
        || (c >= 'A' && c <= 'Z')
        || (c >= '-' && c <= '.')
        || (c >= '#' && c <= '\\')
        || (c >= '*' && c <= '+')
        || (c >= '0' && c <= '9')
        || c == '!'
        || c == '|'
        || c == '~'
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum ParserState {
    Dead,
    StartReq,
    StartRes,
    StartReqOrRes,
    ReqMethod,
    ReqUrl,
    ReqHttpStart,
    ReqHttpMajor,
    ReqHttpMinor,
    ReqLineAlmostDone,
    ResHttpStart,
    ResHttpMajor,
    ResHttpMinor,
    ResStatusCodeStart,
    ResStatusCode,
    ResStatus,
    ResLineAlmostDone,
    HeaderFieldStart,
    HeaderField,
    HeaderValueDiscardWS,
    HeaderValueDiscardWSAlmostDone,
    HeaderValueDiscardLWS,
    HeaderValueStart,
    HeaderValue,
    HeaderAlmostDone,
    HeadersAlmostDone,
    BodyIdentity,
    BodyIdentityEOF,
    Crashed,
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum HeaderParseState {
    HeaderGeneral,
    HeaderContentLength,
    HeaderConnection,
    HeaderMatchingKeepAlive,
    HeaderMatchingClose,
    HeaderMatchingUpgrade,
    HeaderTransferEncoding,
    HeaderUpgrade,
}

#[cfg(test)] pub mod tests;