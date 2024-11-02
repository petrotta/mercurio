A SysMLv2 command line interpreter that implements the OMG emerging reusable asset specfication, designed for SysML CI/CD development.

The major features:
* Inspection of SysML libraries with commands like `mercurio.exe inspect -dir sysml_folder`
* Creation of a SysML library with `mercurio.exe create my_new_module`
* Ability to use modular packages -- SysML that includes a package description and dependencies:
~~~
      <package>
        <org>Example Org</org>
        <project>Project 1</project>
        <version>1.0.0</version>
        <description/>
        <dependencies/>
      </package>
~~~
 

Compilation: 

* Create a gradle.properties file, with permissions to download packages from github packages:
  
gpr.user=<username>
gpr.key=<key>

  
* From Gradle, run:
  
  gradle createExe

Execution:

Example:

Inspects all source files in the pilot implementation:
.\build\launch4j\mercurio.exe inspect -dir C:\dev\git\SysML-v2-Pilot-Implementation\sysml\src

