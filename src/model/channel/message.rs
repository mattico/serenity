use serde_json::builder::ObjectBuilder;
use std::mem;
use ::constants;
use ::client::rest;
use ::model::*;
use ::utils::builder::{CreateEmbed, CreateMessage};

#[cfg(feature="cache")]
use ::client::CACHE;

impl Message {
    /// Deletes the message.
    ///
    /// **Note**: The logged in user must either be the author of the message or
    /// have the [Manage Messages] permission.
    ///
    /// # Errors
    ///
    /// If the `cache` feature is enabled, then returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required permissions.
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [`ClientError::InvalidUser`]: ../client/enum.ClientError.html#variant.InvalidUser
    /// [Manage Messages]: permissions/constant.MANAGE_MESSAGES.html
    pub fn delete(&self) -> Result<()> {
        #[cfg(feature="cache")]
        {
            let req = permissions::MANAGE_MESSAGES;
            let is_author = self.author.id == CACHE.read().unwrap().user.id;
            let has_perms = utils::user_has_perms(self.channel_id, req)?;

            if !is_author && !has_perms {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        self.channel_id.delete_message(self.id)
    }

    /// Deletes all of the [`Reaction`]s associated with the message.
    ///
    /// **Note**: Requires the [Manage Messages] permission.
    ///
    /// # Errors
    ///
    /// If the `cache` feature is enabled, then returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required permissions.
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [`Reaction`]: struct.Reaction.html
    /// [Manage Messages]: permissions/constant.MANAGE_MESSAGES.html
    pub fn delete_reactions(&self) -> Result<()> {
        #[cfg(feature="cache")]
        {
            let req = permissions::MANAGE_MESSAGES;

            if !utils::user_has_perms(self.channel_id, req)? {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        rest::delete_message_reactions(self.channel_id.0, self.id.0)
    }

    /// Edits this message, replacing the original content with new content.
    ///
    /// Message editing preserves all unchanged message data.
    ///
    /// Refer to the documentation for [`CreateMessage`] for more information
    /// regarding message restrictions and requirements.
    ///
    /// **Note**: Requires that the current user be the author of the message.
    ///
    /// # Examples
    ///
    /// Edit a message with new content:
    ///
    /// ```rust,ignore
    /// // assuming a `message` has already been bound
    ///
    /// message.edit(|m| m.content("new content"));
    /// ```
    ///
    /// # Errors
    ///
    /// If the `cache` is enabled, returns a [`ClientError::InvalidUser`] if the
    /// current user is not the author.
    ///
    /// Returns a [`ClientError::MessageTooLong`] if the content of the message
    /// is over [`the limit`], containing the number of unicode code points
    /// over the limit.
    ///
    /// [`ClientError::InvalidUser`]: ../client/enum.ClientError.html#variant.InvalidUser
    /// [`ClientError::MessageTooLong`]: ../client/enum.ClientError.html#variant.MessageTooLong
    /// [`CreateMessage`]: ../utils/builder/struct.CreateMessage.html
    /// [`the limit`]: ../utils/builder/struct.CreateMessage.html#method.content
    pub fn edit<F>(&mut self, f: F) -> Result<()>
        where F: FnOnce(CreateMessage) -> CreateMessage {
        #[cfg(feature="cache")]
        {
            if self.author.id != CACHE.read().unwrap().user.id {
                return Err(Error::Client(ClientError::InvalidUser));
            }
        }

        let mut builder = CreateMessage::default();

        if !self.content.is_empty() {
            builder = builder.content(&self.content);
        }

        if let Some(embed) = self.embeds.get(0) {
            builder = builder.embed(|_| CreateEmbed::from(embed.clone()));
        }

        if self.tts {
            builder = builder.tts(true);
        }

        let map = f(builder).0;

        match rest::edit_message(self.channel_id.0, self.id.0, &Value::Object(map)) {
            Ok(edited) => {
                mem::replace(self, edited);

                Ok(())
            },
            Err(why) => Err(why),
        }
    }

    /// Returns message content, but with user and role mentions replaced with
    /// names and everyone/here mentions cancelled.
    #[cfg(feature="cache")]
    pub fn content_safe(&self) -> String {
        let mut result = self.content.clone();

        // First replace all user mentions.
        for u in &self.mentions {
            result = result.replace(&u.mention(), &u.distinct());
        }

        // Then replace all role mentions.
        for id in &self.mention_roles {
            let mention = id.mention();

            if let Some(role) = id.find() {
                result = result.replace(&mention, &format!("@{}", role.name));
            } else {
                result = result.replace(&mention, "@deleted-role");
            }
        }

        // And finally replace everyone and here mentions.
        result.replace("@everyone", "@\u{200B}everyone")
              .replace("@here", "@\u{200B}here")
    }

    /// Gets the list of [`User`]s who have reacted to a [`Message`] with a
    /// certain [`Emoji`].
    ///
    /// The default `limit` is `50` - specify otherwise to receive a different
    /// maximum number of users. The maximum that may be retrieve at a time is
    /// `100`, if a greater number is provided then it is automatically reduced.
    ///
    /// The optional `after` attribute is to retrieve the users after a certain
    /// user. This is useful for pagination.
    ///
    /// **Note**: Requires the [Read Message History] permission.
    ///
    /// [`Emoji`]: struct.Emoji.html
    /// [`Message`]: struct.Message.html
    /// [`User`]: struct.User.html
    /// [Read Message History]: permissions/constant.READ_MESSAGE_HISTORY.html
    #[inline]
    pub fn get_reaction_users<R, U>(&self, reaction_type: R, limit: Option<u8>, after: Option<U>)
        -> Result<Vec<User>> where R: Into<ReactionType>, U: Into<UserId> {
        self.channel_id.get_reaction_users(self.id, reaction_type, limit, after)
    }

    /// Returns the associated `Guild` for the message if one is in the cache.
    ///
    /// Returns `None` if the guild's Id could not be found via [`guild_id`] or
    /// if the Guild itself is not cached.
    ///
    /// Requires the `cache` feature be enabled.
    ///
    /// [`guild_id`]: #method.guild_id
    #[cfg(feature="cache")]
    pub fn guild(&self) -> Option<Arc<RwLock<Guild>>> {
        self.guild_id().and_then(|guild_id| CACHE.read().unwrap().get_guild(guild_id))
    }

    /// Retrieves the Id of the guild that the message was sent in, if sent in
    /// one.
    ///
    /// Returns `None` if the channel data or guild data does not exist in the
    /// cache.
    #[cfg(feature="cache")]
    pub fn guild_id(&self) -> Option<GuildId> {
        match CACHE.read().unwrap().get_channel(self.channel_id) {
            Some(Channel::Guild(ch)) => Some(ch.read().unwrap().guild_id),
            _ => None,
        }
    }

    /// True if message was sent using direct messages.
    #[cfg(feature="cache")]
    pub fn is_private(&self) -> bool {
        match CACHE.read().unwrap().get_channel(self.channel_id) {
            Some(Channel::Group(_)) | Some(Channel::Private(_)) => true,
            _ => false,
        }
    }

    /// Checks the length of a string to ensure that it is within Discord's
    /// maximum message length limit.
    ///
    /// Returns `None` if the message is within the limit, otherwise returns
    /// `Some` with an inner value of how many unicode code points the message
    /// is over.
    pub fn overflow_length(content: &str) -> Option<u64> {
        // Check if the content is over the maximum number of unicode code
        // points.
        let count = content.chars().count() as i64;
        let diff = count - (constants::MESSAGE_CODE_LIMIT as i64);

        if diff > 0 {
            Some(diff as u64)
        } else {
            None
        }
    }

    /// Pins this message to its channel.
    ///
    /// **Note**: Requires the [Manage Messages] permission.
    ///
    /// # Errors
    ///
    /// If the `cache` is enabled, returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required permissions.
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [Manage Messages]: permissions/constant.MANAGE_MESSAGES.html
    pub fn pin(&self) -> Result<()> {
        #[cfg(feature="cache")]
        {
            let req = permissions::MANAGE_MESSAGES;

            if !utils::user_has_perms(self.channel_id, req)? {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        self.channel_id.pin(self.id.0)
    }

    /// React to the message with a custom [`Emoji`] or unicode character.
    ///
    /// **Note**: Requires the [Add Reactions] permission.
    ///
    /// # Errors
    ///
    /// If the `cache` is enabled, returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required [permissions].
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [`Emoji`]: struct.Emoji.html
    /// [Add Reactions]: permissions/constant.ADD_REACTIONS.html
    /// [permissions]: permissions
    pub fn react<R: Into<ReactionType>>(&self, reaction_type: R) -> Result<()> {
        #[cfg(feature="cache")]
        {
            let req = permissions::ADD_REACTIONS;

            if !utils::user_has_perms(self.channel_id, req)? {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        rest::create_reaction(self.channel_id.0,
                              self.id.0,
                              &reaction_type.into())
    }

    /// Replies to the user, mentioning them prior to the content in the form
    /// of: `@<USER_ID>: YOUR_CONTENT`.
    ///
    /// User mentions are generally around 20 or 21 characters long.
    ///
    /// **Note**: Requires the [Send Messages] permission.
    ///
    /// **Note**: Message contents must be under 2000 unicode code points.
    ///
    /// # Errors
    ///
    /// If the `cache` is enabled, returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required permissions.
    ///
    /// Returns a [`ClientError::MessageTooLong`] if the content of the message
    /// is over the above limit, containing the number of unicode code points
    /// over the limit.
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [`ClientError::MessageTooLong`]: ../client/enum.ClientError.html#variant.MessageTooLong
    /// [Send Messages]: permissions/constant.SEND_MESSAGES.html
    pub fn reply(&self, content: &str) -> Result<Message> {
        if let Some(length_over) = Message::overflow_length(content) {
            return Err(Error::Client(ClientError::MessageTooLong(length_over)));
        }

        #[cfg(feature="cache")]
        {
            let req = permissions::SEND_MESSAGES;

            if !utils::user_has_perms(self.channel_id, req)? {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        let mut gen = self.author.mention();
        gen.push_str(": ");
        gen.push_str(content);

        let map = ObjectBuilder::new()
            .insert("content", gen)
            .insert("tts", false)
            .build();

        rest::send_message(self.channel_id.0, &map)
    }

    /// Unpins the message from its channel.
    ///
    /// **Note**: Requires the [Manage Messages] permission.
    ///
    /// # Errors
    ///
    /// If the `cache` is enabled, returns a
    /// [`ClientError::InvalidPermissions`] if the current user does not have
    /// the required permissions.
    ///
    /// [`ClientError::InvalidPermissions`]: ../client/enum.ClientError.html#variant.InvalidPermissions
    /// [Manage Messages]: permissions/constant.MANAGE_MESSAGES.html
    pub fn unpin(&self) -> Result<()> {
        #[cfg(feature="cache")]
        {
            let req = permissions::MANAGE_MESSAGES;

            if !utils::user_has_perms(self.channel_id, req)? {
                return Err(Error::Client(ClientError::InvalidPermissions(req)));
            }
        }

        rest::unpin_message(self.channel_id.0, self.id.0)
    }
}

impl From<Message> for MessageId {
    /// Gets the Id of a `Message`.
    fn from(message: Message) -> MessageId {
        message.id
    }
}
