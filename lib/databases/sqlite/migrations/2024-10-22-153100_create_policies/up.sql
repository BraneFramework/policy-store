-- Your SQL goes here
 CREATE TABLE policies (
    version bigint PRIMARY KEY NOT NULL,
    name Text NOT NULL,
    description Text NOT NULL,
    creator Text NOT NULL,
    created_at BigInt NOT NULL,
    content Text NOT NULL
 );

-- Your SQL goes here
 CREATE TABLE active_version (
    version bigint NOT NULL,
    activated_on DATETIME NOT NULL,
    activated_by TEXT NOT NULL,
    deactivated_on DATETIME NULL;
    deactivated_by TEXT NULL;
    FOREIGN KEY(version) REFERENCES policies(version)
    PRIMARY KEY (version, activated_on)
 );