A SysMLv2 command line interpreter, designed to run with entirely native code.  It can be compiled to native Windows, Linux, or Apple binaries.  Once compiled into a single exe, there are not other dependencies (including JDK, etc).  

Compilation: 

* Install GraalVM 23
* Create a gradle.properties file, with permissions to download packages from github packages:
  
gpr.user=<username>
gpr.key=<key>

  
* From Gradle, run:
  
  gradle nativeCompile

Execution:

  .\build\native\nativeCompile\mercurio.exe validate -src "C:\dev\git\SysML-v2-Pilot-Implementation\sysml\src\examples\Flashlight Example\"


