# The Big Idea

A SysMLv2 command line interpreter designed for SysML CI/CD development, enabling software-like workflows for model development.  This includes features like model validation, test case execution, and deployment tasks (e.g. Markdown generation).

* [Simple one-click installation](https://github.com/petrotta/mercurio/releases/download/latest/mercurio.exe) with platform native executables (Windows, Mac (tbd), Linux (tbd))
* No configuration required.  Installs with default sysml libraries.
* Modular, allowing plugins for additional features

After installation, try from the command line (it will add to path automatically):

> mercurio version

 

# The major features:
* Create a SysML project with: `mercurio create -dir newProject123`
* Inspection of SysML models with commands like `mercurio inspect -dir newProject123`
* Ability to use modular packages -- SysML that includes a package description (package.xml) and dependencies:
~~~
      <package>
        <org>Example Org</org>
        <project>Project 1</project>
        <version>1.0.0</version>
        <description/>
        <dependencies/>
      </package>
~~~
* Modular plugins to extend functionality

`mercurio run "example plugin"` 
~~~ 
@PluginAnnotation(name = "example plugin", version = "1.0.2")
public class MyPlugin extends Plugin {
    @Override
    public void init() {
    }

    @Override
    public void run(Project project) {
        System.out.println(project.validate().toString());
    }
}
~~~

# Technical Details:
On Windows, mercurio will create a USER.HOME/.mercurio folder:

~~~
.mercurio
      plugins
      stdlib
            sysml.library
                  Domain Libraries
                  Kernal Libraries
                  System Libraries
~~~
   
# Compilation: 
* Requires Hydraulic Conveyor for packaging 
* Create a gradle.properties file, with permissions to download packages from github packages:
      gpr.user=<username>
      gpr.key=<key>
* From Gradle, run:
  gradle conveyorMakeApp





