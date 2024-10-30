package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.Project;

import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;


@CommandLine.Command(name = "run", mixinStandardHelpOptions = true,
        description = "Runs scripts.")
public class Run extends ProjectCommand implements Callable<Integer> {
    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception {

        Project project = initProject(sourceDir, libDir, verbose);

//        Binding binding = new Binding();
//       // binding.setVariable("foo", new Integer(2));
//        GroovyShell shell = new GroovyShell(binding);
//
//        Object value = shell.evaluate("1+1");
//        System.out.println(value);

        return 0;
    }
}
