if (-not (Test-Path -Path Env:\TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN)) {
    Write-Host "[Get-DiscordBotToken] Make sure to dot-source this file!"
    $password = op read "op://Private/Teamy-Archiver Discord bot token/credential" --no-newline
    $env:TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN = $password
}