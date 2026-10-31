on run argv
    set itemPath to item 1 of argv
    set theItem to POSIX file itemPath
    try
        activate
        set actionChoice to choose from list {"Open", "Open With…", "Reveal in Finder", "Get Info"} with title "System actions" with prompt itemPath
        if actionChoice is false then return
        set actionName to item 1 of actionChoice
        if actionName is "Open With…" then
            set selectedApp to choose application with prompt "Open this item with:" as alias
            tell application "Finder" to open theItem using selectedApp
        else if actionName is "Reveal in Finder" then
            tell application "Finder"
                reveal theItem
                activate
            end tell
        else if actionName is "Get Info" then
            tell application "Finder"
                open information window of theItem
                activate
            end tell
        else
            tell application "Finder" to open theItem
        end if
    on error errorText number errorNumber
        if errorNumber is not -128 then error errorText number errorNumber
    end try
end run
