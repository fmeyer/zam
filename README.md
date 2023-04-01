# zam

zsh alias manager

```

USAGE:
    zam <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    add        Add a new alias
    aliases    List all aliases in shell `eval` ready format
    export     Export aliases to a CSV file
    help       Print this message or the help of the given subcommand(s)
    import     Import aliases from a CSV file
    remove     Remove an alias
    update     Update an existing alias
```

### Setup 

Add the following to the `~/.zshrc` file

importing aliases 

    source <(zam aliases)

importing secrets (from 1password)

    source <(zam secrets)

Please note that `op` has to be properly configured to use the secrets function