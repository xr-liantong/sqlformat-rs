select ID, NAME, EMAIL from USERS where STATUS = 'active' and SCORE > 10 order by CREATED_AT desc limit 5;
SELECT A.ID, (select MAX(TOTAL) from ORDERS O where O.CUSTOMER_ID = A.ID) as MAX_TOTAL from ACCOUNTS A where A.REGION in ('eu','us');
-- leading comment line
select /* inline block */ COUNT(*) from SESSIONS
group by CHANNEL having COUNT(*) > 3;
update TICKETS set STATUS='closed', OWNER_ID=42 where ID in (1,2,3) and NOTE is not null;
insert into events (ID, KIND, PAYLOAD) values (1, 'click', 'x--y'), (2, 'view', NULL);
select case when QTY > 0 then PRICE*QTY else 0 end as AMOUNT, COALESCE(SKU,'') from PRODUCTS P join SHIPMENTS S on P.ID = S.SKU where P.PRICE between 1 and 99;
delete from SESSIONS where CREATED_AT < '2026-01-01' and STATUS <> 'keep';
select distinct CURRENCY, SUM(AMOUNT) over (partition by CURRENCY order by ID) from PAYMENTS;