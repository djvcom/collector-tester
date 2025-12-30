# OpenTelemetry Collector Configuration Testing

This project is designed to enable testing of an OpenTelemetry Collector in isolation. By using the <TESTCONTAINERS? crate> and the mock-collector crate, you can assert on:

- collector attribute processing
- routing and filtering
- failover
- any other config in between receivers and exporters
