<?php
$identifier = $_GET["i"] ?? false;
$check_code = $_GET["c"] ?? false;

if (!$identifier || !$check_code)
	die();
$db = new mysqli("localhost", "user", "password", "database");
$query = sprintf("INSERT INTO confirm VALUES ('%s', '%s')", $db->real_escape_string($identifier), $db->real_escape_string($check_code));
$db->query($query);
