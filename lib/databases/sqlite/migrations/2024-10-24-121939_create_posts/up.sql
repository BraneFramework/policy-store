-- Your SQL goes here
CREATE TABLE `policies`(
	`version` BIGINT NOT NULL PRIMARY KEY,
	`name` TEXT NOT NULL,
	`description` TEXT NOT NULL,
	`creator` TEXT NOT NULL,
	`created_at` TIMESTAMP NOT NULL,
	`content` TEXT NOT NULL
);

CREATE TABLE `active_version`(
	`version` BIGINT NOT NULL,
	`activated_on` TIMESTAMP NOT NULL,
	`activated_by` TEXT NOT NULL,
	`deactivated_on` TIMESTAMP,
	`deactivated_by` TEXT,
	PRIMARY KEY(`version`, `activated_on`)
);

