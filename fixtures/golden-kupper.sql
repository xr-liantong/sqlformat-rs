SELECT id, name, Email FROM users WHERE status = 'active' AND score > 10 ORDER BY created_at DESC LIMIT 5;
SELECT a.id, (SELECT max(total) FROM orders o WHERE o.customer_id = a.id) AS max_total FROM accounts a WHERE a.region IN ('eu','us');
-- leading comment line
SELECT /* inline block */ count(*) FROM sessions
GROUP BY channel HAVING count(*) > 3;
UPDATE tickets SET status='closed', owner_id=42 WHERE id IN (1,2,3) AND note IS NOT NULL;
INSERT INTO EVENTS (id, kind, payload) VALUES (1, 'click', 'x--y'), (2, 'view', NULL);
SELECT CASE WHEN qty > 0 THEN price*qty ELSE 0 END AS amount, coalesce(sku,'') FROM products p JOIN shipments s ON p.id = s.sku WHERE p.price BETWEEN 1 AND 99;
DELETE FROM sessions WHERE created_at < '2026-01-01' AND status <> 'keep';
SELECT DISTINCT currency, sum(amount) OVER (PARTITION BY currency ORDER BY id) FROM payments;