drop table if exists "dms";

create table if not exists "dms" (
    id serial primary key not null,
    chunk integer not null,
    pos integer not null,
    condition text not null,
    aa text not null,
    log2_fold_change double precision not null,
    log2_std_error double precision not null,
    statistic double precision not null,
    p_value double precision not null,
    version text not null,
    protein text not null,
    created_at timestamptz not null
)
