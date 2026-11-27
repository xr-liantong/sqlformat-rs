select a + 1, a-1, a * -1, a * (b + c), (a + b) * 2, - a * b from t;
select a = b, a <> b, a >= b, a <= b, a != b, not a = b from t where a between 1 and 2;
select * from t;
select t.*, count(*), sum(a) from t group by 1;
select x::int, y || 'z', json_col -> 'k' from t;