package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import org.omg.sysml.interactive.SysMLInteractive;
import org.omg.sysml.interactive.SysMLInteractiveResult;

import org.omg.sysml.lang.sysml.Element;
import org.omg.sysml.lang.sysml.Namespace;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "eval", mixinStandardHelpOptions = true,
        description = "Compile the source code.")
public class Evaluate extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories")
    private File sourceDir;


    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory",arity = "0..1")
    //@CommandLine.Parameters(index = "0", description = "The file or directory to check.")
    private File libDir;

    @CommandLine.Option(names = { "-i", "--input" }, paramLabel = "INPUT", description = "eval input")
    private String input;

    @CommandLine.Option(names = { "-t", "--target" }, paramLabel = "TARGET", description = "eval target")
    private String target;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception {
        Project project = initProject(sourceDir, libDir, verbose);

        project.readSysML();

        Application.console("# resources read: " + project.getResourceSet().getResources().size());
        Application.console( project.eval(input, target));

        return 0;
    }

    public List<Element> process(SysMLInteractive instance, String input) {

        SysMLInteractiveResult result = instance.process(input);

        Element root = result.getRootElement();

        return ((Namespace)root).getOwnedMember();
    }


}
