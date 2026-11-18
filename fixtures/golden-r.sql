select id,
       name,
       Email
from users
where status = 'active'
  and score > 10
order by created_at desc
limit 5;


SELECT a.id,

  (select max(total)
   from orders o
   where o.customer_id = a.id) as max_total
from accounts a
where a.region in ('eu',
                   'us');

-- leading comment line

select /* inline block */ count(*)
from sessions
group by channel
having count(*) > 3;


update tickets
set status='closed',
    owner_id=42
where id in (1,
             2,
             3)
  and note is not null;


insert into events (id,
                    kind,
                    payload)
values (1, 'click', 'x--y'),
       (2, 'view', NULL);


select case
           when qty > 0 then price*qty
           else 0
       end as amount,
       coalesce(sku, '')
from products p
join shipments s on p.id = s.sku
where p.price between 1 and 99;


delete
from sessions
where created_at < '2026-01-01'
  and status <> 'keep';


select distinct currency,
                sum(amount) over (partition by currency
                                  order by id)
from payments;