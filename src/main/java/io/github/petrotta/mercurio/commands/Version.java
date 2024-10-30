package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.Application;
import picocli.CommandLine;

import java.util.concurrent.Callable;

@CommandLine.Command(name = "version", mixinStandardHelpOptions = true,
        description = "Returns version of mercurio")
public class Version implements Callable<Integer> {

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        System.out.println("Version: " + Application.APP_VERSION);
        return 0;
    }
}