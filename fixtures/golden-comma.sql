select alpha_one
     , beta_two
     , gamma_three
     , delta_four
     , epsilon_five
     , zeta_six
     , eta_seven
from some_table
where x = 1;


select case
           when a = 1 then 'one'
           when a = 2 then 'two'
           else 'many'
       end as label
     , count(*)
from t
group by 1;


insert into t (a, b, c, d, e)
values (1, 'two', 3.0, null, 'five')
     , (6, 'seven', 8.0, null, 'nine');


select coalesce(a, b, c, d, e, f, g, h) as merged
     , upper(trim(note))
from t;


select a
     , b
     , c
from t1
union all
select d
     , e
     , f
from t2
order by 1
limit 10;


update t
set a = 1
  , b = 2
  , c = 3
  , d = 4
where id in (1
           , 2
           , 3
           , 4
           , 5
           , 6
           , 7
           , 8
           , 9
           , 10);

with x as
  (select 1 as a
        , 2 as b
        , 3 as c)
select a
     , b
     , c
from x;