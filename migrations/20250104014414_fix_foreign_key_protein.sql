-- Add migration script here
-- Step 2: Add a new column "protein_id" as a foreign key
ALTER TABLE "dms"
ADD COLUMN "protein_id" INTEGER;

-- Step 3: Add a foreign key constraint to reference the "proteins" table
ALTER TABLE "dms" ADD CONSTRAINT "fk_protein_id" FOREIGN KEY ("protein_id") REFERENCES "proteins" ("id") ON DELETE CASCADE;

-- Step 4 (Optional): Populate the "proteins" table and migrate data
-- If you already have data in the "dms" table:
-- 4a. Insert unique proteins from "dms" into "proteins"
INSERT INTO
    "proteins" ("protein")
SELECT DISTINCT
    "protein"
FROM
    "dms";

-- 4b. Update the "dms" table to reference the new "protein_id"
UPDATE "dms"
SET
    "protein_id" = (
        SELECT
            "id"
        FROM
            "proteins"
        WHERE
            "proteins"."protein" = "dms"."protein"
    );

-- 4c. Drop the old "protein" column after migration
ALTER TABLE "dms"
DROP COLUMN "protein";
