<!DOCTYPE html>
<html lang="en">

<head>
  <meta charset="UTF-8">
  <link rel="stylesheet" href="/index.css">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <script src="https://cdnjs.cloudflare.com/ajax/libs/crypto-js/3.1.2/rollups/aes.js"></script>
  <title> Rolladensoftware - Sicher TM</title>
</head>

<script type="text/javascript" id="myscript">
  function encrypt() {
    document.getElementById("payload").value = CryptoJS.AES.encrypt("CMD" + new Date().getTime() + ":" + document.getElementById("command").value, document.getElementById("password").value);
  }

  function decrypt(a) {
    console.log(CryptoJS.AES.decrypt(document.getElementById("command").value, document.getElementById("password").value).toString(CryptoJS.enc.Utf8));
  }
</script>

<body>
  <?php if (isset($_POST["payload"])) {
    $db = new mysqli("localhost", "user", "password", "database");
    $db->query(sprintf("INSERT INTO commands VALUES (NOW(), '%s')", $db->real_escape_string($_POST["payload"])));
  } ?>
  <form method="POST">
    <h2>Enter Command v1</h2>
    <input id="command" onchange="encrypt()" placeholder="Befehl" type="text">
    <input id="password" onchange="encrypt()" placeholder="Kennwort" type="text">
    <input id="payload" name="payload" type="hidden">
    <button type="submit">send</button>
  </form>
</body>

</html>
