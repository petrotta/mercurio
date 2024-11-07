
$env:Path += ';../../build/packages/bin/'
$ts = Get-Date -Format "yyyyMMddHHmm"

$folderForTest = -join("../../tmp/test", $ts)
mercurio version
mercurio create -dir $folderForTest
mercurio validate -dir $folderForTest
mercurio inspect -dir $folderForTest

$folderForTest = "C:\dev\git\SysML-v2-Pilot-Implementation\sysml\src"
mercurio version
mercurio create -dir $folderForTest
mercurio validate -dir $folderForTest
mercurio inspect -dir $folderForTest