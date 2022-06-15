<?php

$db = new mysqli("localhost", "user", "password", "database");
foreach ($db->query("SELECT * FROM commands")->fetch_all() as $row)
  echo $row[1] . "\n";
