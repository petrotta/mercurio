package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.BuildSystem;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.Application;
import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "create", mixinStandardHelpOptions = true,
        description = "Creates an empty sysml package.")
public class Create implements Callable<Integer>  {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        if(sourceDir == null) {
            sourceDir = Application.getCurrentDir();;
        }
        console("Source Dir:   "+ sourceDir.toString(), verbose);

        if(!sourceDir.exists()) {
            sourceDir.mkdir();
            Application.setCurrentDir(sourceDir);

            File srcDir = new File(sourceDir, "src");
            srcDir.mkdir();

            File sysmlFile = new File(srcDir, "Example.sysml");
            sysmlFile.createNewFile();

            PackageManifest packageManifest = new PackageManifest();
            packageManifest.setOrg("Example Org");
            packageManifest.setProject("Project 1");
            packageManifest.setVersion("1.0.0");


            File packageFile = new File(sourceDir, "package.xml");
            BuildSystem.writeBuildFile(packageManifest, packageFile);
        } else {
            System.out.println("Directory already exists.");
        }



        return 0;
    }

}
