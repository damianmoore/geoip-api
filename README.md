# GeoIP API

A lightweight, self-hosted REST API service that provides city-level geolocation lookups using DB-IP's free city database. Built with Rust for optimal performance and minimal resource footprint.

## Features

- 🚀 **Lightweight**: Minimal memory usage and container size
- 🔄 **Auto-updating**: Automatically downloads and manages monthly database updates
- 🐳 **Docker-ready**: Multi-architecture Docker images (AMD64/ARM64)
- 🛡️ **Secure**: Distroless container images with minimal attack surface
- 📊 **Rich data**: Returns city, country, coordinates, timezone, and accuracy information
- 🔧 **Easy deployment**: Simple Make targets and GitHub Actions CI/CD

## Quick Start

### Using Make

```bash
# Clone the repository
git clone https://github.com/your-username/geoip-api.git
cd geoip-api

# Build and run
make docker-build
make docker-run
```

## API Usage

### Lookup IP Address

```bash
# Basic lookup
curl http://localhost/8.8.8.8

# Response
{
  "ip": "8.8.8.8",
  "city": "Mountain View",
  "country": "United States",
  "country_code": "US",
  "latitude": 37.4056,
  "longitude": -122.0775,
  "timezone": "America/Los_Angeles",
  "accuracy_radius": 1000
}
```

### Health Check

```bash
curl http://localhost/health
# Response: {"status":"healthy"}
```

## Configuration

### Environment Variables

- `DATA_DIR`: Directory for database storage (default: `/data`)

### Command Line Options

```bash
./geoip-api --help

Options:
  --bind <BIND>          Bind address [default: 0.0.0.0:80]
  --data-dir <DATA_DIR>  Data directory [default: /data]
```

## Database Management

The service automatically manages the GeoIP database:

- Downloads DB-IP free city database on startup if not present
- Checks for new monthly database once per day
- Maintains the 3 most recent database files
- Uses atomic symlink updates for zero-downtime database switches
- Validates file size before switching to prevent corruption

## Development

### Prerequisites

- Rust 1.89+ (for local development)
- Docker (for containerized development)
- Make

### Local Development

```bash
# Setup development environment
make dev-setup

# Run in development mode
make docker-run-dev

# View logs
make docker-logs

# Run tests
make test
```

### Building

```bash
# Build Docker image
make docker-build

# Build multi-architecture images
make docker-multiarch

# Build locally (requires Rust toolchain)
make build
```

## Deployment

### Docker Compose

```yaml
version: '3.8'
services:
  geoip-api:
    image: your-username/geoip-api:latest
    ports:
      - "80:80"
    volumes:
      - geoip-data:/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "/usr/local/bin/geoip-api", "--help"]
      interval: 30s
      timeout: 3s
      retries: 3

volumes:
  geoip-data:
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: geoip-api
spec:
  replicas: 2
  selector:
    matchLabels:
      app: geoip-api
  template:
    metadata:
      labels:
        app: geoip-api
    spec:
      containers:
      - name: geoip-api
        image: your-username/geoip-api:latest
        ports:
        - containerPort: 80
        volumeMounts:
        - name: data
          mountPath: /data
        resources:
          requests:
            memory: "128Mi"
            cpu: "50m"
          limits:
            memory: "256Mi"
            cpu: "200m"
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: geoip-data
```

## CI/CD

The project includes GitHub Actions workflow that:

- Builds multi-architecture Docker images (AMD64/ARM64)
- Pushes to Docker Hub on main branch
- Tags images with semantic versioning
- Uses build cache for faster builds

### Required Secrets

- `DOCKER_USERNAME`: Docker Hub username
- `DOCKER_PASSWORD`: Docker Hub password/token

## Performance

- **Memory usage**: ~50-100MB depending on database size
- **Startup time**: ~2-5 seconds
- **Request latency**: <10ms for cached lookups
- **Container size**: ~15-20MB (distroless)

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Acknowledgments

- [DB-IP](https://db-ip.com/) for providing the free GeoIP database
- [MaxMind](https://www.maxmind.com/) for the database format specification
- Rust community for excellent crates and tooling
