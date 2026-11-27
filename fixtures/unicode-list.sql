select 姓名, 城市, '表情：👨‍👩‍👧‍👦 🇨🇳 ⚡' as emoji,
       'עברית: שלום עולם' as rtl, "表格名" as quoted_name
from 用户表 where 城市 = '北京' and 姓名 like '张%';
