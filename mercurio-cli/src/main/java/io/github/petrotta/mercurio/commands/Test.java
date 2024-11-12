package io.github.petrotta.mercurio.commands;


//import org.graalvm.polyglot.Context;
//import org.graalvm.polyglot.Value;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import io.github.petrotta.mercurio.plugins.PluginUtils;
import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;


@CommandLine.Command(name = "test", mixinStandardHelpOptions = true,
        description = "Test.")
public class Test extends ProjectCommand implements Callable<Integer> {
    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;



    @Override
    public Integer call() throws Exception {

        Project project = initProject(sourceDir, libDir, verbose);

        project.readSysML();
        console("Resources read: " + project.getResourceSet().getResources().size());


        if(project instanceof StructuredProject) {
            ((StructuredProject) project).loadDependencies();
            project.runTests();

        }




        return 0;
    }
}
