drop table if exists "dms";

create table if not exists "dms" (
    id integer primary key not null,
    chunk integer,
    pos integer,
    condition text,
    aa text,
    log2_fold_change float,
    log2_std_error float,
    statistic float,
    p_value float,
    version text,
    total_bc float,
    total_bc_sum float
)
