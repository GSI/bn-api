ALTER TABLE domain_actions
    alter column main_table set not null;

ALTER TABLE domain_actions
    alter column main_table_id set not null;