package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import org.eclipse.xtext.validation.Issue;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "validate", mixinStandardHelpOptions = true,
        description = "Checks the source files for compile errors.")
public class Validate extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        Project project = initProject(sourceDir, libDir, verbose);

        project.readSysML();
        if(project instanceof StructuredProject) {
            ((StructuredProject) project).loadDependencies();
        }

        console("Resources read: " + project.getResourceSet().getResources().size());

        List<Issue> issues = project.validate();
        console("Total issues: " + issues.size());
        for(Issue issue : issues) {
            console(issue.toString());
        }

        return 0;
    }




}