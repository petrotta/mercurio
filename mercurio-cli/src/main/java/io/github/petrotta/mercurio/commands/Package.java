package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.BuildSystem;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.utils.ZipUtils;
import io.github.petrotta.mercurio.Application;

import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;
@CommandLine.Command(name = "package", mixinStandardHelpOptions = true,
        description = "Packages up a sysml package")


public class Package extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception {

        File userDir = new File(System.getProperty("user.dir"));
        if(sourceDir == null) {
            sourceDir = userDir;
        }

        PackageManifest pkgFile = BuildSystem.readBuildFile(new File(sourceDir, Application.PACKAGE_MANIFEST_FILENAME));
        System.out.println("org: " + pkgFile.getOrg() + ", project: " +  pkgFile.getProject() + ", version: "+ pkgFile.getVersion());


//        for(String d : pkgFile.getDepend()) {
//             System.out.println("Dep: " + d);
//        }


        System.out.println(Application.getProperties());

        System.out.println("User Dir: " + userDir.getAbsolutePath());
        System.out.println("Source Dir:   "+ sourceDir.toString());


        System.out.println("Package Dir:   "+ Application.getPackagesDir());
        ZipUtils.zip(sourceDir,new File(Application.getPackagesDir(), "test_a_0.0.1.zip"));


        return 0;
    }
}
