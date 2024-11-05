package io.github.petrotta.mercurio.utils;

import org.apache.log4j.*;

public class Logging {

    public static ConsoleAppender createConsoleAppender() {
        ConsoleAppender console = new ConsoleAppender(); //create appender
        //configure the appender
        String PATTERN = "%d [%p|%c|%C{1}] %m%n";
        console.setLayout(new PatternLayout(PATTERN));
        console.setThreshold(Level.FATAL);
        console.activateOptions();

        return console;
        //add appender to any Logger (here is root)
//        Logger.getRootLogger().addAppender(console);


    }


}
