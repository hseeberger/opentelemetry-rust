use crate::{ExportConfig, OTEL_EXPORTER_OTLP_TRACES_ENDPOINT, OTEL_EXPORTER_OTLP_TRACES_TIMEOUT};
use std::str::FromStr;
use std::{fmt, time::Duration};
use tonic::metadata::MetadataMap;
use tonic::transport::Channel as TonicChannel;
#[cfg(feature = "tls")]
use tonic::transport::ClientTlsConfig;

#[derive(Clone)]
pub struct Channel<T>(pub(crate) T);

impl<T> fmt::Debug for Channel<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Channel").finish()
    }
}

/// Configuration for [tonic]
///
/// [tonic]: https://github.com/hyperium/tonic
#[derive(Debug, Default)]
pub struct TonicConfig {
    /// Custom metadata entries to send to the collector.
    pub metadata: Option<MetadataMap>,

    /// TLS settings for the collector endpoint.
    #[cfg(feature = "tls")]
    pub tls_config: Option<ClientTlsConfig>,
}

/// Build a trace exporter that uses [tonic] as grpc layer and opentelemetry protocol.
///
/// It allows users to
/// - add additional metadata
/// - set tls config(with `tls` feature enabled)
/// - bring custom [channel]
///
/// [tonic]: <https://github.com/hyperium/tonic>
/// [channel]: tonic::transport::Channel
#[derive(Debug)]
pub struct TonicExporterBuilder<T> {
    pub(crate) exporter_config: ExportConfig,
    pub(crate) tonic_config: TonicConfig,
    pub(crate) channel: Result<Channel<T>, crate::Error>,
}

impl<T> TonicExporterBuilder<T> {
    /// Set the TLS settings for the collector endpoint.
    #[cfg(feature = "tls")]
    pub fn with_tls_config(mut self, tls_config: ClientTlsConfig) -> Self {
        self.tonic_config.tls_config = Some(tls_config);
        self
    }

    /// Set custom metadata entries to send to the collector.
    pub fn with_metadata(mut self, metadata: MetadataMap) -> Self {
        self.tonic_config.metadata = Some(metadata);
        self
    }

    /// Use `channel` as tonic's transport channel.
    /// this will override tls config and should only be used
    /// when working with non-HTTP transports.
    ///
    /// Users MUST make sure the [`ExportConfig::timeout`] is
    /// the same as the channel's timeout.
    pub fn with_channel<C>(self, channel: Channel<C>) -> TonicExporterBuilder<C> {
        TonicExporterBuilder {
            exporter_config: self.exporter_config,
            tonic_config: self.tonic_config,
            channel: Ok(channel),
        }
    }
}

impl Default for TonicExporterBuilder<TonicChannel> {
    fn default() -> Self {
        Self {
            exporter_config: ExportConfig::default(),
            tonic_config: TonicConfig::default(),
            channel: default_channel(),
        }
    }
}

fn default_channel() -> Result<Channel<TonicChannel>, crate::Error> {
    let export_config = ExportConfig::default();

    let endpoint_str = match std::env::var(OTEL_EXPORTER_OTLP_TRACES_ENDPOINT) {
        Ok(val) => val,
        Err(_) => format!("{}{}", export_config.endpoint, "/v1/traces"),
    };

    let endpoint = TonicChannel::from_shared(endpoint_str)?;

    let _timeout = match std::env::var(OTEL_EXPORTER_OTLP_TRACES_TIMEOUT) {
        Ok(val) => match u64::from_str(&val) {
            Ok(seconds) => Duration::from_secs(seconds),
            Err(_) => export_config.timeout,
        },
        Err(_) => export_config.timeout,
    };

    #[cfg(feature = "tls")]
    let tonic_channel = match TonicConfig::default().tls_config.as_ref() {
        Some(tls_config) => endpoint.tls_config(tls_config.clone())?,
        None => endpoint,
    }
    .timeout(_timeout)
    .connect_lazy();

    #[cfg(not(feature = "tls"))]
    let tonic_channel = endpoint.timeout(_timeout).connect_lazy();

    Ok(Channel(tonic_channel))
}
