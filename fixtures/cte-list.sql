with active_users as (
  select id, name from users where status = 'active'
), totals as (
  select user_id, sum(amount) as total from orders group by user_id having sum(amount) > 100
)
select a.id, a.name, coalesce(t.total, 0) as total
from active_users a left join totals t on t.user_id = a.id
order by total desc;

with recursive tree(id, parent_id, depth) as (
  select id, parent_id, 1 from nodes where parent_id is null
  union all
  select n.id, n.parent_id, t.depth + 1 from nodes n join tree t on n.parent_id = t.id
)
select * from tree where depth <= 5 order by depth, id;
