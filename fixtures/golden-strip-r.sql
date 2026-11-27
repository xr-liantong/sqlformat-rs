select a,
       b
from t
where x = 'c--d'
  and y = 2;

/*+ index(t idx) */
select count(*)
from t;

--+ parallel(4)

select 3;