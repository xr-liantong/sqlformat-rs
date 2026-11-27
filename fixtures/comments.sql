-- leading comment
/* block
   spanning lines */
select a, b /* between columns */ from t
where x = 'c--d' -- trailing comment
  and y = 2;
/*+ index(t idx) */ select count(*) from t;
--+ parallel(4)
select 3;
