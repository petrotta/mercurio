package io.github.petrotta.mercurio.commands;


//import org.graalvm.polyglot.Context;
//import org.graalvm.polyglot.Value;
import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import io.github.petrotta.mercurio.plugins.PluginManager;
import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;


@CommandLine.Command(name = "run", mixinStandardHelpOptions = true,
        description = "Runs scripts.")
public class Run extends ProjectCommand implements Callable<Integer> {
    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @CommandLine.Parameters(index = "0", description = "Task to run")
    private String task;

    @Override
    public Integer call() throws Exception {

        Project project = Application.openProject(sourceDir, libDir, verbose);

        project.readSysML();
        if(project instanceof StructuredProject) {
            ((StructuredProject) project).loadDependencies();
        }

        console("Resources read: " + project.getResourceSet().getResources().size());

        PluginManager mgr = new PluginManager();
        mgr.index();




        if(mgr.getTaskPluginClass(task)==null) {
            console("Couldn't find task: " + task);
            return 1;
        } else {
            mgr.run(task, project);
            return 0;
        }


    }
}
