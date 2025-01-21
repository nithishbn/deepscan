-- Add up migration script here
CREATE TABLE protein (
    id SERIAL PRIMARY KEY,
    name VARCHAR(30) NOT NULL,
    pdb_id VARCHAR(10)
);

CREATE TABLE variant (
    id SERIAL PRIMARY KEY,
    chunk INTEGER NOT NULL,
    pos INTEGER NOT NULL,
    condition VARCHAR(30) NOT NULL,
    aa VARCHAR(30) NOT NULL,
    log2_fold_change DOUBLE PRECISION NOT NULL,
    log2_std_error DOUBLE PRECISION NOT NULL,
    statistic DOUBLE PRECISION NOT NULL,
    p_value DOUBLE PRECISION NOT NULL,
    version VARCHAR(30) NOT NULL,
    protein_id INTEGER NOT NULL REFERENCES protein (id) ON DELETE CASCADE,
    created_on TIMESTAMP NOT NULL
);
