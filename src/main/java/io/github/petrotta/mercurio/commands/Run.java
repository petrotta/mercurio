package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.build.Project;


//import org.graalvm.polyglot.Context;
//import org.graalvm.polyglot.Value;
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

//       // Project project = initProject(sourceDir, libDir, verbose);
//
//        try (Context context = Context.create()) {
//            Value function = context.eval("js", "x => x+1");
//            assert function.canExecute();
//            int x = function.execute(41).asInt();
//            assert x == 42;
//        }
//
//        try (Context context = Context.create()) {
//            Value function = context.eval("python", "print('Hello Python!')");
//            //assert function.canExecute();
//            //int x = function.execute().asInt();
//            //assert x == 42;
//            //int x = function.execute(41).asInt();
//            //assert x == 42;
//        }


        return 0;
    }
}
