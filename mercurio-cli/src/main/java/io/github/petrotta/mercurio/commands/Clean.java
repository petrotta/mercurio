package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.utils.ZipUtils;
import picocli.CommandLine;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "package", mixinStandardHelpOptions = true,
        description = "Packages up a sysml package")


public class Clean extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;







    @Override
    public Integer call() throws Exception {

        File libDir = Application.getStdLibDir();
        Project project = initProject(sourceDir, libDir, false);

        if(!(project instanceof StructuredProject)) {
            console("Clean only works on structured projects.");
            return 0;
        }

        StructuredProject structurePrj = (StructuredProject) project;
        File buildDir = structurePrj.getBuildDir();
        Files.deleteIfExists(buildDir.toPath());


        return 0;
    }
}
