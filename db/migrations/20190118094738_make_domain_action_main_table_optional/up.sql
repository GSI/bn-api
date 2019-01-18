ALTER TABLE domain_actions
    alter column main_table drop not null;

ALTER TABLE domain_actions
    alter column main_table_id drop not null;
