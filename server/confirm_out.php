<?php

$db = new mysqli("localhost", "user", "password", "database");
foreach ($db->query("SELECT * FROM confirm")->fetch_all() as $row)
  $lines[] = $row[0] . ":" . $row[1];
echo implode("\n", $lines);
