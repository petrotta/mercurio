package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.BuildSystem;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.utils.FileUtils;
import io.github.petrotta.mercurio.utils.TimeUtils;
import io.github.petrotta.mercurio.utils.ZipUtils;
import io.github.petrotta.mercurio.Application;

import picocli.CommandLine;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "package", mixinStandardHelpOptions = true,
        description = "Packages up a sysml package")


public class Package extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-install", "-i" }, paramLabel = "INSTALL", description = "install to the local mercurio cache", arity = "0..1")
    private boolean install = false;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;



    @Override
    public Integer call() throws Exception {

        Project project = Application.openProject(sourceDir, libDir, verbose);

        if(!(project instanceof StructuredProject)) {
            console("Packaging only works on structured projects.");
            return 0;
        }
        
        StructuredProject structuredProject = (StructuredProject) project;
        structuredProject.readSysML();
        
        structuredProject.makeBuild(install);


        System.out.println("Package Dir:   "+ Application.getPackagesDir());



        return 0;
    }


}
