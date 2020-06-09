Ext.define('pbs-datastore-statistics', {
    extend: 'Ext.data.Model',

    fields: [
	'store', 'total', 'used', 'avail', 'estimated-full-date', 'history',
	{
	    name: 'usage',
	    calculate: function(data) {
		let used = data.used || 0;
		let total = data.total || 0;
		if (total > 0) {
		    return used/total;
		} else {
		    return -1;
		}
	    }
	},
    ],

    proxy: {
        type: 'proxmox',
	url: "/api2/json/status/datastore-usage"
    },
    idProperty: 'store',
});

Ext.define('PBS.DatastoreStatistics', {
    extend: 'Ext.grid.Panel',
    alias: 'widget.pbsDatastoresStatistics',

    title: gettext('Datastore Usage'),

    emptyText: gettext('No Data'),

    controller: {
	xclass: 'Ext.app.ViewController',

	render_estimate: function(value) {
	    if (!value) {
		return gettext('Not enough data');
	    }

	    let now = new Date();
	    let estimate = new Date(value*1000);

	    let timespan = (estimate - now)/1000;

	    if (+estimate <= +now || isNaN(timespan)) {
		return gettext('Never');
	    }

	    let duration = Proxmox.Utils.format_duration_human(timespan);
	    return Ext.String.format(gettext("in {0}"), duration);
	},

	init: function(view) {
	    Proxmox.Utils.monStoreErrors(view, view.getStore().rstore);
	},
    },

    columns: [
	{
	    text: gettext('Name'),
	    dataIndex: 'store',
	    sortable: true,
	},
	{
	    text: gettext('Size'),
	    dataIndex: 'total',
	    sortable: true,
	    width: 90,
	    renderer: Proxmox.Utils.format_size,
	},
	{
	    text: gettext('Used'),
	    dataIndex: 'used',
	    sortable: true,
	    width: 90,
	    renderer: Proxmox.Utils.format_size,
	},
	{
	    text: gettext('Usage %'),
	    dataIndex: 'usage',
	    sortable: true,
	    xtype: 'widgetcolumn',
	    widget: {
		xtype: 'progressbarwidget',
		bind: '{record.usage}',
		textTpl: [
		    '<tpl if="value &gt;= 0">',
		    '{value:number("0.00")*100}%',
		    '<tpl else>',
		    Proxmox.Utils.unknownText,
		    '</tpl>',
		],
	    },
	},
	{
	    text: gettext('Estimated Full'),
	    dataIndex: 'estimated-full-date',
	    sortable: true,
	    renderer: 'render_estimate',
	    flex: 1,
	    minWidth: 130,
	    maxWidth: 200,
	},
	{
	    text: gettext("History (last Month)"),
	    width: 100,
	    xtype: 'widgetcolumn',
	    dataIndex: 'history',
	    flex: 1,
	    widget: {
		xtype: 'sparklineline',
		bind: '{record.history}',
		spotRadius: 0,
		fillColor: '#ddd',
		lineColor: '#555',
		lineWidth: 0,
		chartRangeMin: 0,
		chartRangeMax: 1,
		tipTpl: '{y:number("0.00")*100}%'
	    }
	},
    ],

    store: {
	type: 'diff',
	autoDestroy: true,
	autoDestroyRstore: true,
	sorters: 'store',
	rstore: {
	    type: 'update',
	    storeid: 'pbs-datastore-statistics',
	    model: 'pbs-datastore-statistics',
	    autoStart: true,
	    interval: 30000,
	},
    },

})
