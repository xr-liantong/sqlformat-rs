-- a plain comment
--+ a hint comment
# mysql style comment
SELECT `backtick name`, :named, ?, %(py)s, @user_var, @@global_var, $1, ${dollar}
FROM "quoted table" AS t
WHERE a::text = 'x' AND b := 3 AND c != d AND e <> f AND g <= h
  AND i ~ 'regex' AND j !~* 're2' AND k = ANY(ARRAY[1,2]) AND l IS NOT NULL
  AND m IN (1, 2, 3) AND n BETWEEN -1.5e+10 AND .5 AND o LIKE '%a\_b%' ESCAPE '\'
  AND p = x'1F' AND q = 0xDEAD AND r = b'1010' AND s = N'nat' AND u = E'esc\n'
  AND v = CAST(w AS TIMESTAMP WITH TIME ZONE) AND x = INTERVAL '1 day'
/* block comment */ AND y = 1e-7 AND z = -0.0 AND aa = 1_000
SELECT /*+ inline hint */ CASE WHEN t.a > 1 THEN 'y' ELSE 'n' END AS flag, COUNT(*) OVER (PARTITION BY t.a ORDER BY t.b ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) FROM t
CREATE TABLE demo (id INTEGER PRIMARY KEY, amount DECIMAL(10, 2), note VARCHAR(64), ts TIMESTAMP);
WITH RECURSIVE cte(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM cte WHERE n < 5) SELECT * FROM cte;
INSERT INTO demo VALUES (1, 2.5, 'hi', CURRENT_TIMESTAMP), (2, NULL, 'there', NOW());
UPDATE demo SET amount = amount * 1.1 WHERE id = 1;
DELETE FROM demo WHERE id = 2;
