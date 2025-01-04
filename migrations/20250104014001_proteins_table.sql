-- Add migration script here
drop table if exists "proteins";

create table if not exists "proteins" (
    id serial primary key not null,
    protein text not null
)
