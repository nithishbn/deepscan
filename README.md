# DMS Viewer

A high-performance web application for visualizing and analyzing Deep Mutational Scanning (DMS) data, built with Rust and modern web technologies.

## Overview

DMS Viewer is a comprehensive tool for exploring protein variant effects from deep mutational scanning experiments. It provides interactive heatmaps, scatter plots, and 3D protein structure visualization to help researchers understand the functional impact of amino acid substitutions across entire protein sequences.

### Key Features

- **Interactive Heatmap Visualization**: Color-coded amino acid position matrix showing variant effects
- **Scatter Plot Analysis**: Statistical exploration of variant data with brushing and filtering
- **3D Protein Structure Integration**: PDBe Molstar integration for structural context
- **Real-time Data Filtering**: Dynamic filtering by significance, effect size, and statistical measures
- **High-performance Backend**: Rust-based server with PostgreSQL database for fast queries
- **Responsive Design**: HTMX-powered frontend for seamless user interaction

## Architecture

### Backend (Rust)
- **Framework**: Axum web framework with Tokio async runtime
- **Database**: PostgreSQL with SQLx for type-safe queries
- **Templating**: Maud for server-side HTML generation
- **Data Processing**: CSV parsing and bulk database operations

### Frontend
- **Interactivity**: HTMX for dynamic content updates
- **Visualization**: D3.js for scatter plots and data visualization
- **3D Structures**: PDBe Molstar plugin for protein structure viewing
- **Styling**: Custom CSS with responsive design
- **State Management**: Alpine.js for client-side reactivity

### Database Schema

The application uses a PostgreSQL schema with two main tables:

```sql
-- Protein information
CREATE TABLE protein (
    id SERIAL PRIMARY KEY,
    name VARCHAR(30) NOT NULL,
    pdb_id VARCHAR(10)
);

-- Variant data from DMS experiments
CREATE TABLE variant (
    id SERIAL PRIMARY KEY,
    chunk INTEGER NOT NULL,                    -- Experimental chunk/batch
    pos INTEGER NOT NULL,                      -- Amino acid position
    condition VARCHAR(30) NOT NULL,            -- Experimental condition
    aa VARCHAR(30) NOT NULL,                   -- Amino acid substitution
    log2_fold_change DOUBLE PRECISION NOT NULL, -- Effect size
    log2_std_error DOUBLE PRECISION NOT NULL,   -- Standard error
    statistic DOUBLE PRECISION NOT NULL,        -- Z-statistic
    p_value DOUBLE PRECISION NOT NULL,          -- Statistical significance
    version VARCHAR(30) NOT NULL,              -- Data version
    protein_id INTEGER REFERENCES protein(id), -- Foreign key
    created_on TIMESTAMP NOT NULL              -- Upload timestamp
);
```

## Data Format

The application accepts TSV files with the following required columns:

| Column | Type | Description |
|--------|------|-------------|
| `chunk` | Integer | Experimental batch identifier |
| `pos` | Integer | Amino acid position in sequence |
| `condition` | String | Experimental condition name |
| `aa` | String | Amino acid single-letter code |
| `log2FoldChange` | Float | Effect size (log2 fold change) |
| `log2StdError` | Float | Standard error of effect size |
| `statistic` | Float | Z-statistic |
| `p.value` | Float | Statistical significance |
| `version` | String | Data version identifier |

### Example Data Structure

```tsv
chunk	pos	condition	aa	log2FoldChange	log2StdError	statistic	p_value	version
1	174	condition_1	*	-0.111	0.235	-0.764	0.05	v3.0.0
1	174	condition_1	A	0.111	0.186	0.350	0.253	v3.0.0
1	174	condition_2	*	0.354	0.137	2.430	0.112	v3.0.0
```

## Installation & Setup

### Prerequisites

- Rust (latest stable)
- PostgreSQL 13+
- Nix (optional, for development environment)

### Development Setup with Nix

The project includes a Nix flake for reproducible development environments:

```bash
# Enter development shell
nix develop

# Database will be automatically initialized in ./db/
# Start PostgreSQL (if not auto-started)
pg_ctl start

# Run database migrations
sqlx migrate run

# Start the development server
cargo run --bin server
```

### Manual Setup

1. **Database Setup**:
```bash
# Create PostgreSQL database
createdb dms_viewer

# Set environment variables
export DATABASE_URL="postgresql://username:password@localhost/dms_viewer"
export PORT=3000

# Run migrations
sqlx migrate run
```

2. **Build and Run**:
```bash
# Development
cargo run --bin server

# Production build
cargo build --release
./target/release/server
```

### Docker Deployment

```bash
# Build container
docker build -t dms-viewer .

# Run with environment variables
docker run -p 3000:80 \
  -e DATABASE_URL=postgresql://user:pass@host/db \
  dms-viewer
```

## Usage

### Web Interface

1. **Select Protein**: Choose from available proteins in the database
2. **Choose Condition**: Select experimental condition to visualize
3. **Configure Visualization**:
   - **Position Filter**: Order by significance, effect size, or no ordering
   - **Paint By**: Color code by p-value, log2 fold change, or z-statistic
   - **Threshold**: Filter variants by statistical significance
4. **Explore Data**:
   - **Heatmap**: Interactive amino acid substitution matrix
   - **Scatter Plot**: Statistical analysis with brushing capabilities
   - **3D Structure**: Structural context via PDBe Molstar

### API Endpoints

- `GET /` - Main application interface
- `GET /proteins` - List available proteins
- `GET /conditions?protein=<name>` - Get conditions for protein
- `GET /variants` - Fetch variant data with filtering
- `GET /variant/:id` - Get specific variant details
- `GET /plot?plot=<type>` - Generate heatmap or scatter plot

## Configuration

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string (required)
- `PORT`: Server port (default: 3000)

### PDB Structure Mapping

The application includes hardcoded PDB mappings for common proteins:
- GLP1R: 7ki0
- GIPR: 8wa3
- RHO: 1f88
- Default: 7s15

## Data Visualization Features

### Heatmap View
- Color-coded amino acid substitution matrix
- Position-wise organization with amino acids grouped by properties
- Real-time filtering and threshold adjustment
- Hover effects showing detailed statistics
- Lazy loading for performance with large datasets (500 positions per page)

### Scatter Plot Analysis
- Statistical exploration of variant effects
- Interactive data point inspection
- Configurable color coding by statistical measures

### 3D Structure Integration
- PDBe Molstar plugin for structure visualization
- Position highlighting synchronized with data selection
- Structural context for understanding variant effects

## Development

### Project Structure

```
├── src/
│   ├── lib.rs              # Core data structures and utilities
│   └── server/
│       ├── main.rs         # Web server and route handlers
│       └── utils.rs        # HTTP utilities and middleware
├── assets/                 # Frontend assets
│   ├── style.css          # Application styles
│   ├── viewer.js          # 3D structure viewer integration
│   ├── scatter.js         # D3.js scatter plot implementation
│   └── htmx.min.js        # HTMX library
├── migrations/             # Database schema migrations
├── Cargo.toml            # Rust dependencies
├── Dockerfile            # Container configuration
├── flake.nix             # Nix development environment
└── pyproject.toml        # Python dependencies (Alembic)
```

### Key Dependencies

**Rust Backend:**
- `axum` (0.7.9) - Modern async web framework
- `sqlx` (0.8.2) - Type-safe database queries with PostgreSQL support
- `tokio` (1.0) - Async runtime
- `maud` - Compile-time HTML templates
- `serde` (1.0.217) - Serialization framework
- `csv` (1.3.1) - TSV/CSV parsing

**Frontend:**
- `htmx` - Dynamic HTML interactions
- `d3.js` - Data visualization
- `alpine.js` - Reactive components
- `pdbe-molstar` (3.3.0) - 3D protein structure viewer

### Database Migrations

The project uses SQLx for database migrations:

```bash
# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

### Performance Optimizations

- **Lazy Loading**: Paginated data loading (PAGE_SIZE: 500)
- **Database Indexing**: Optimized queries for fast filtering
- **Caching**: HTTP cache headers for static assets (max-age=3600)
- **Async Processing**: Non-blocking I/O with Tokio runtime
- **Compiled Templates**: Maud for efficient HTML generation

## Deployment

### Production Build

```bash
cargo build --release --bin server
```

### Docker Compose Example

```yaml
version: '3.8'
services:
  app:
    build: .
    ports:
      - "3000:80"
    environment:
      - DATABASE_URL=postgresql://postgres:password@db:5432/dms_viewer
    depends_on:
      - db

  db:
    image: postgres:15
    environment:
      - POSTGRES_DB=dms_viewer
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
```

The application exposes port 80 in the Docker container and serves static assets from the `/assets` directory.
